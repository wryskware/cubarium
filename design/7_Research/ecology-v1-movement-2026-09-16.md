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

## Build and commits

| | |
| --- | --- |
| Branch | `worktree-agent-a962bfc09ff0bb810` (a worktree of `main`) |
| Parent | `b70624d` |
| Pre-registration commit | `3934eb7` — everything above the rule, written before a row existed |
| Test-authoring commit | `7206fc7` — the fifteen definition tests against a stub; 14 of 15 red |
| Implementation commit | `18cace4` — the measures, the price in the box, the `--prices` axis |
| Result commit | *(this note, below)* |
| Build stamp in every row | **`18cace4f97e0`** — clean, not `-dirty`: nothing was uncommitted in the row-producing build |
| Host | 32 logical cores, 91 GiB; 8 workers, as the brief caps |

`cubarium-core` is not modified; `crates/cubarium-search/src/es/` is not modified.
The files this workstream touched are `params.rs`, `evaluate.rs`, `calibrate.rs`,
the new `movement.rs`, its one-line module declaration in `lib.rs`, the
`--prices` dispatch in `main.rs`, and `tests/movement_measures.rs`.

### Tests

| Command | Result |
| --- | --- |
| `cargo test --release -p cubarium-search` | **108 passed**, 0 failed (77 unit, 4 `es_repair`, 12 `harness`, **15 `movement_measures`**) |
| `cargo test --release -p cubarium-core` | **467 passed**, 0 failed, 2 ignored — the same numbers workstream A recorded, which is what "core untouched" has to look like |
| `graft build` | 6,597 nodes, 13,684 edges, clean |

**22 tests are new**: the 15 definition tests in `tests/movement_measures.rs`, 4
unit tests in `movement.rs` (the per-cell counter reproduces A's aggregate rule
on the same series; a recovery implies an earlier depletion; a body that never
returns does not pull the mean interval to zero; a surviving skimmer has no loss
tick), and 3 in `calibrate.rs` (the control price is the shipped one and all
three levels are inside the declared box; writing the default price changes no
configuration bit while a raised one does; a price outside the box is refused
before compute is spent).

The authoring order is on the record rather than asserted: commit `7206fc7`
carries the fifteen tests against a module that returns nothing, and 14 of the
15 fail there. The one that passes — "an unwatched cell never crosses" — passes
trivially against a counter that counts nothing, and that is said in the commit
message rather than counted as a pass.

**A build hazard, recorded because it can silently produce a wrong answer.** The
worktree shares `CARGO_TARGET_DIR` with the main checkout. Cargo overwrote the
`cubarium-search` rlib between the two source trees twice during this session,
and the second time a `cargo test --release` linked the *other* tree's library
and failed with `unresolved import cubarium_search::movement`. A rebuild fixed
it both times. Every number in this note comes from a binary that printed build
stamp `18cace4f97e0`, listed `--prices` in `calibrate --help`, and reported
`14 searched parameters` — all three checked before the campaign was launched.

## Exact commands

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo build --release -p cubarium-search
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test  --release -p cubarium-search   # 108 passed
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test  --release -p cubarium-core     # 467 passed, 2 ignored

# preflight, the no-op check
./target/release/cubarium-search calibrate-export --candidate baseline  --seed 1 \
  --why "movement preflight: the no-op check" --out runs/ecology-v1-movement/preflight
./target/release/cubarium-search calibrate-export --candidate fast-leaf --seed 1 --selected \
  --why "movement preflight: the no-op check" --out runs/ecology-v1-movement/preflight

# the campaign: 3 prices x 2 configurations x 6 training seeds x 3 apex arms
./target/release/cubarium-search calibrate --stage matrix \
  --candidates baseline,fast-leaf --seed-set training --seeds 6 --arms 0,1,2 \
  --prices 0.00036,0.0018,0.006 \
  --ticks 180000 --sample-every 600 --introduce-tick 6000 \
  --workers 8 --wall-seconds 1080 --out runs/ecology-v1-movement
```

`runs/ecology-v1-movement/matrix/{evals.jsonl,summary.json}`, 108 rows.

## The two reproduction checks, before anything was interpreted

**1. The no-op check passes, and more strongly than the pre-registration asked.**
`calibrate-export` at seed 1 printed config hash **`fc1aefa33ebd70a1`** for
`baseline` and **`09e244392ec91768`** for `fast-leaf` — A's recorded hashes — and
the exported TOMLs are **byte-identical** to
`runs/ecology-v1-calibration/selected/{baseline,fast-leaf}.toml`. Adding
`organism.move_cost` to the parameter box and writing it at its shipped default
changes no bit of a configuration.

**2. The row check passes on all 36 control rows.** Every
`(candidate, seed, arm)` at `move_cost = 0.00036` carries the same
`metrics.final_state_hash` as A's retained screen row: **36 of 36, 0
mismatches**. The recorder was rewritten around the spatial measures, so the
same check was run on A's own spatial number as well:
`cells_per_body_window_late` differs from A's screen by **exactly 0** on all 36.
The control arm is the screen's world, bit for bit, and the new measures did not
disturb the old one. The control cells also reproduce A's gate pattern — the
`baseline` cells fail only `guilds_intact` in all three arms and the `fast-leaf`
cells are plausible in all three.

Two internal identities hold in all 108 rows: the per-cell crossing counter's
totals equal A's aggregate `depletion_events` / `recovery_events` counters, and
`cells_cycled == cells_recovered` — a recovery never appears without an earlier
depletion in the same cell.

## The matrix

108 trials, **320.4 s = 5.3 wall minutes** on 8 workers, 0 skipped, 58,729
ticks/s, peak RSS 21 MiB, 0 refused configurations, 0 failures. Each row of the
table is a mean over 18 runs (6 training seeds x 3 apex arms).

| cand | `move_cost` | cells/body (late) | residence s | revisit s | depletions | recoveries | runs >= 1 rec | still depleted at the end | late pop | SP/SP0 | births | deaths | late starv | late age | motor % of bill | feeding % | kinds at end | worlds lost | **verdict** |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline | **0.00036** | 288.9 | 6.07 | 226.7 | 14.6 | 0.17 | 3/18 | 14.4 of 14.6 | 43.4 | 2.43 | 171.5 | 156.1 | 9.9 | 15.2 | 10.9 % | 49.4 | 2.11 | 0 | *control* |
| baseline | 0.0018 | **52.1** | 12.35 | 150.8 | **55.9** | 0.33 | 6/18 | 55.6 of 55.7 | 33.9 | 3.73 | 184.2 | 173.8 | 24.1 | 0.2 | 19.3 % | 72.0 | **1.00** | 0 | **PARTIAL** |
| baseline | 0.006 | **43.8** | 16.16 | 182.3 | 45.7 | 0.28 | 5/18 | 45.4 of 45.6 | 24.4 | 3.75 | 154.7 | 157.2 | 22.1 | 0.0 | 37.6 % | 81.8 | **1.00** | **3** | **PARTIAL** |
| fast-leaf | **0.00036** | 369.1 | 4.22 | 260.3 | 0.7 | 0.00 | 0/18 | 0.7 of 0.7 | 59.4 | 2.11 | 195.1 | 157.1 | 7.6 | 22.4 | 12.0 % | 47.0 | 3.00 | 0 | *control* |
| fast-leaf | 0.0018 | **173.8** | 9.29 | 203.3 | 12.2 | 0.11 | 2/18 | 12.1 of 12.1 | 43.4 | 3.41 | 194.8 | 172.9 | 16.7 | 1.1 | 27.7 % | 68.7 | 1.67 | 0 | **PARTIAL** |
| fast-leaf | 0.006 | **54.9** | 13.64 | 172.8 | 22.3 | 0.00 | 0/18 | 22.3 of 22.3 | 37.4 | 4.26 | 192.5 | 179.1 | 26.8 | 0.1 | 40.0 % | 79.7 | **1.00** | **1** | **PARTIAL** |

Column key: *cells/body*, *residence* and *revisit* are the late window, over
bodies that met the 90 % coverage rule; *depletions*, *recoveries* and *still
depleted at the end* are whole-run, over 1,112 watched cells of 1,280; *late
pop* is the late-window mean prey population; *births* and *deaths* are
whole-run prey; *late starv* and *late age* are late-window deaths by cause;
*motor % of bill* is `(body_bill_total - body_bill_upkeep) / body_bill_total`
over the late window, i.e. what share of the complete body bill the motor and
senses took; *kinds at end* is founder forms alive at the horizon, of 4;
*worlds lost* is worlds that ended empty.

**Censoring, stated rather than averaged away.** The four worlds that ended
empty stopped inside their fifth window, so they closed no late window and no
late spatial window. The `move_cost = 0.006` spatial and late columns are
therefore means over **15 of 18** rows for `baseline` and **17 of 18** for
`fast-leaf`; every other cell is 18 of 18. The `worlds lost` column carries them
and the verdict rule reads that column, so a collapse is counted where it
matters rather than dropped from a mean.

### The verdicts against the pre-registered rules

| configuration | R: range < 0.60 x | D: repeated depletion **and** recovery | S: starvation collapse | U: range unchanged | K: deaths >= 1.25 x | verdict |
| --- | --- | --- | --- | --- | --- | --- |
| baseline x 0.0018 | **yes** (52.1 vs 173.3) | **no** (dep 55.9 pass, rec 0.33 fail, 6/18 fail) | no (33.9 vs 21.7, 0 lost) | no | no (174 vs 156) | **PARTIAL** |
| baseline x 0.006 | **yes** (43.8 vs 173.3) | **no** (dep 45.7 pass, rec 0.28 fail, 5/18 fail) | **yes** (3 worlds lost) | no | no (157 vs 156) | **PARTIAL** |
| fast-leaf x 0.0018 | **yes** (173.8 vs 221.5) | **no** (dep 12.2 pass, rec 0.11 fail, 2/18 fail) | no (43.4 vs 29.7, 0 lost) | no | no (173 vs 157) | **PARTIAL** |
| fast-leaf x 0.006 | **yes** (54.9 vs 221.5) | **no** (dep 22.3 pass, rec 0.00 fail, 0/18 fail) | **yes** (1 world lost) | no | no (179 vs 157) | **PARTIAL** |

**No cell is CONFIRMED and no cell is REFUTED.** The range half of the
confirmation rule is met decisively and monotonically in all four; the
depletion/recovery half fails in all four, and it fails on the **recovery**
clause alone.

**A defect in my own pre-registration, disclosed.** The `D` rule's depletion
clause (`>= 2.0` crossings per run) was already satisfied by the `baseline`
control at 14.6, so it never discriminated for that configuration; only the
`fast-leaf` control (0.7) sat below it. The rule still did its job, because it
is conjunctive and the recovery clause is what decided every cell, but a
threshold the control already clears is a weak threshold, and setting it that
way was my error rather than a result.

## What the price knob actually did

1. **It buys range, monotonically, and it is the largest effect in the
   campaign.** Late-window cells per body falls 288.9 -> 52.1 -> 43.8 (baseline)
   and 369.1 -> 173.8 -> 54.9 (`fast-leaf`). Residence per cell rises 6.1 ->
   12.3 -> 16.2 s and 4.2 -> 9.3 -> 13.6 s, and the revisit interval *falls*
   (226.7 -> 150.8 s; 260.3 -> 172.8 s) — bodies stay longer and come back
   sooner, which is spatial concentration in both of the ways the
   pre-registration said to look for it. The per-window series shows it is not a
   late-run artefact: `baseline` at 0.0018 reads `— 49.7 46.2 50.4 52.1` cells
   per body across the five windows against the control's `223.9 300.0 336.4
   349.1 288.9`. (The first window is blank at the raised prices because no body
   survived 90 % of it, which is itself finding 4 below.)
2. **It buys local depletion.** Whole-run depletion crossings rise 14.6 -> 55.9
   (baseline, 3.8 x) and 0.7 -> 22.3 (`fast-leaf`, 31 x). Astra's finding 4
   asked whether high range *dilutes* grazing pressure; on this evidence it
   does, and charging for range concentrates it.
3. **It buys very little recovery, and this campaign cannot say why.**
   (Corrected after review; the first version said "no recovery at all" and
   called depletion "an absorbing state", which the raw counters contradict.)
   Recoveries are 0.00–0.33 per run against 0.7–55.9 depletions: **13 recovery
   crossings in the 72 raised-price runs** (baseline 6 at 0.0018 and 5 at 0.006;
   `fast-leaf` 2 at 0.0018 and 0 at 0.006), against 3 in the baseline control.
   Rare, not absent. `still depleted at the end` is within 0.2 of `cells ever
   depleted` in every cell, but a cell can recover and be depleted again (the
   `fast-leaf` 0.0018 seed-1006 arms do), so that equality does not show the
   state is absorbing. Meanwhile the world as a whole goes from 2.1 x to 4.3 x
   its opening foliage, the stands stay alive (late alive cells 1,113–1,128
   against 1,112 at the opening) and plant deaths stay at 0.3–4.8 per run — but
   that greening can happen in other watched cells or in the 168 cells that
   opened without foliage and are not watched. **Why a depleted cell stays below
   50 % of its opening foliage is not resolved here**: it may be slow regrowth
   (`L*mu` marginal), or consumers returning to bite it again, or both. This
   campaign recorded neither the depleted cells' own `L*mu` and trajectory nor
   their post-depletion visits and bites. The supported statement is: the
   price concentrates grazing and multiplies depletion crossings; robust
   repeated recovery was not produced inside 150 minutes; the mechanism is
   open. What would settle it: per depleted cell, `L*mu`, `P/P0` through time,
   time and stock at the last consumer visit, post-depletion visit count and
   served material, and first recovery / re-depletion times.
4. **At every price tested it kills the founder grazers before they breed, and
   in three of the four raised cells the gliders too; that is the dominant
   ecological effect.** (Corrected after review: the first version said both
   herbivore rigs produce zero offspring in all 72 raised-price runs and called
   the outcome a burrower monoculture everywhere. The census table below
   contradicts that for `fast-leaf` at 0.0018, where the glider enters 447
   bodies from 90 founders — **357 glider births** — and 101 are alive at the
   horizon, so the glider lineage crossed conception and completed gestation
   there.) The grazer statement holds in all four raised cells: 180 founder
   grazers, **zero** grazer births, mean age at death **179.6 s** at 0.0018 and
   **79.4 s** at 0.006, every one by starvation with a last-observed usable
   store of 0.007–0.013 e and hunger 1.000. Gliders produce zero births in the
   three other raised cells (mean age 184.3 s and 79.5 s). Reproduction needs
   `bud_min_age_seconds` 120 s plus `gestation_seconds` 30 s **and** a reserve
   at `bud_reserve` = 0.7 x `reserve_max`; at 0.006 the bodies do not reach the
   age gate at all. Whether a founder ever reached `bud_reserve` cannot be read
   from last-observed stores (conception escrows the child once the gates
   pass), so "never reached the reserve" is inferred, not recorded. Founder
   kinds alive at the end fall 2.11 -> 1.00 (baseline) and 3.00 -> 1.67 -> 1.00
   (`fast-leaf`), and the foliage rises to 3.7–4.3 x its opening because much
   less eats it. Late-window deaths by age fall from 15.2 and 22.4 to about 0.
5. **The knob is connected, and hard.** The motor and senses take 10.9–12.0 % of
   the complete body bill at the shipped price and **37.6–40.0 %** at 0.006, and
   the feeding fraction rises from 47–49 % to 80–82 % — four bodies in five
   feeding on every sample, which is a world with almost no behavioural slack
   left.
6. **Four worlds died**, all at 0.006 and all in apex-bearing arms: `baseline`
   seeds 1001 and 1003 in arm 1 and seed 1006 in arm 2, at 19.8, 21.0 and 20.1
   simulated minutes; `fast-leaf` seed 1005 in arm 2 at 18.6 minutes. A's screen
   and held-out recorded **zero** extinctions in 306 runs, so these are the first
   whole-world extinctions the ecology v1 harness has produced. Three of four in
   arm 1 and one in arm 2, with none in arm 0, is suggestive and is **not**
   evidence of an apex effect at n = 4; A measured every paired apex difference
   at under 0.2 seed standard deviations.
7. **One price level moved variety in the other direction, and only one.**
   `fast-leaf` at 0.0018 is the single cell in the matrix where the skimmer
   survives: 171 skimmer-rig bodies alive at the horizon across its 18 runs, and
   the rig lost in 9 of 18 runs against 18 of 18 at the same configuration's
   control. It still ends with fewer founder kinds than its control (1.67 against
   3.00), because the grazer is gone; this is a change of composition, not a gain.

## The variety census

Pooled over the 18 runs of each cell, keyed on what each body was when it was
first seen. The founder roster **measured at tick 0** in every run is
`forms [10, 5, 4, 5, 0]` and `diet bins [4, 5, 15]` — grazer 10 (form 0, diet
0.85), glider 5 (form 1, 0.90), burrower 4 (form 2, 0.10), skimmer 5 (form 3,
0.60) — which is the mapping the pre-registration named, confirmed rather than
assumed.

`entered` = bodies present at tick 0 plus bodies born; `survival` = alive at the
horizon over `entered`; mean lifetime is over deaths only.

| configuration | rig | diet bin | guild | entered | deaths | of which starvation | mean lifetime | alive | survival |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| baseline 0.00036 | grazer | 0.65–1.00 | herbivore | 519 | 374 | 363 | 1,652 s | 145 | 28 % |
| baseline 0.00036 | glider | 0.65–1.00 | herbivore | 678 | 421 | 172 | 5,220 s | 257 | 38 % |
| baseline 0.00036 | burrower | 0.00–0.35 | detritivore | 2,032 | 1,746 | 1,712 | 1,146 s | 286 | 14 % |
| baseline 0.00036 | **skimmer** | **0.35–0.65** | generalist | 271 | 259 | **252** | 926 s | 12 | **4 %** |
| baseline 0.00036 | **skimmer** | **0.65–1.00** | generalist | 5 | 0 | 0 | — | 5 | **100 %** |
| fast-leaf 0.00036 | grazer | 0.65–1.00 | herbivore | 854 | 471 | 328 | 3,696 s | 383 | 45 % |
| fast-leaf 0.00036 | glider | 0.65–1.00 | herbivore | 1,072 | 573 | 328 | 5,024 s | 499 | 47 % |
| fast-leaf 0.00036 | burrower | 0.00–0.35 | detritivore | 1,763 | 1,535 | 1,481 | 1,169 s | 228 | 13 % |
| fast-leaf 0.00036 | **skimmer** | **0.35–0.65** | generalist | 235 | 235 | **233** | 816 s | 0 | **0 %** |
| fast-leaf 0.0018 | grazer | 0.65–1.00 | herbivore | 180 | 180 | 180 | 179 s | 0 | 0 % |
| fast-leaf 0.0018 | glider | 0.65–1.00 | herbivore | 447 | 346 | 339 | 1,521 s | 101 | 23 % |
| fast-leaf 0.0018 | burrower | 0.00–0.35 | detritivore | 2,703 | 2,150 | 2,113 | 1,407 s | 553 | 20 % |
| fast-leaf 0.0018 | **skimmer** | **0.35–0.65** | generalist | 533 | 425 | 424 | 1,433 s | 108 | **20 %** |
| fast-leaf 0.0018 | **skimmer** | **0.65–1.00** | herb. + gen. | 75 | 12 | 12 | 2,091 s | 63 | **84 %** |

The full 45-cell census, including the `baseline` raised-price cells and the
`(form, diet bin, guild)` splits omitted here for width, is in every row under
`movement.census.cells`.

Two facts hold across every cell of the matrix. **Starvation is the cause of
essentially every death that is not old age** — 2,383–3,214 starvations per cell
against 2–404 age deaths and 7–40 predations. And **a starved body's
last-observed usable store is 0.002–0.006 e with hunger 1.000**, against
0.51–1.36 e and hunger 0.78–0.84 for a body that died of age. The terminal-store
measure separates the two causes cleanly, which is the check that it is measuring
what it claims to.

## The skimmer: which leg is it?

The brief asks whether the skimmer is lost through its body, its controller, its
habitat, or a lower realised diet yield. The census answers, with one confound
named.

**It is lost to starvation, everywhere.** Of 259 form-3 deaths in the baseline
control, 252 are starvation, 2 age and 5 predation; of 235 in the `fast-leaf`
control, 233 are starvation and 2 predation. The last form-3 body dies at a
median of 15.4 simulated minutes (baseline control) and 54.8 minutes
(`fast-leaf` control) of 150, and the last form-3 death is a starvation in 15 of
16 baseline-control losses and in 18 of 18 `fast-leaf`-control losses.

**Within the rig, diet is strongly associated with survival.** The comparison
that holds the body fixed (but not birth tick, lineage or other loci — see the
confound below):

| | entered | mean lifetime (deaths only) | survival |
| --- | --- | --- | --- |
| form 3, diet bin 0.35–0.65 (`fast-leaf` 0.0018) | 533 | 1,433 s | 20 % |
| form 3, diet bin 0.65–1.00 (`fast-leaf` 0.0018) | 75 | **2,091 s** | **84 %** |
| form 3, diet bin 0.35–0.65 (baseline control) | 271 | 926 s | 4 % |
| form 3, diet bin 0.65–1.00 (baseline control) | 5 | — | 100 % |

The same rig with a foliage-end diet outlives the same rig with the founder's
middling diet by 1.46 x and out-survives it by 4.2 x. The across-rig comparison
*inside one bin* says the same thing from the other direction: in `fast-leaf` at
0.0018, form 3 in bin 0.65–1.00 survives at 84 % while form 1 (the glider) in
the same bin survives at 23 % and form 2 (the burrower) in its own bin at 20 %.
**In the one configuration where skimmers carry a foliage diet they are the
best-surviving bodies in the world.** (Corrected after review: the first
version concluded from this that the body is not the handicap. It cannot: every
foliage-bin skimmer is a descendant, not a randomised founder, so the comparison
mixes later birth, right-censoring, selection into the mutant lineage and
possible mutation at other loci. It is an association, and a strong one.)

**The controller algorithm is the same for every body** — no neural animal, no
quiet policy — but its realised behaviour depends on inherited drives, body
capabilities and habitat, so holding the algorithm constant does not by itself
rule the controller's behaviour out as a cause (corrected after review).

**Habitat is not marked by anything measured here, and is not ruled out.** At
the baseline control, form-3 bodies' late-window range and residence (303 cells
per body, 1.90 s) sit inside the band of the other mobile rigs (grazer 452 cells
and 1.73 s, glider 495 and 1.68 s) and nowhere near the burrower's (48 and
12.12 s). Nothing about how the skimmer uses space marks it out. But its niche is
the algae the wet floor grows (`water.algae_light`), and this campaign recorded
neither the cell classes a body stood in nor its intake by channel, so "the wet
floor is thin" is untested rather than excluded.

**The leading hypothesis is the diet's realised yield, and what it would take
to close it.** (Corrected after review from "the supported reading".) At
`gamma = 1` a body's caps are `cap_foliage = diet` and `cap_detrital = 1 - diet`,
so the founder skimmer at `diet = 0.60` takes 0.60 of the leaf it eats where a
grazer at 0.85 takes 0.85, and 0.40 of the litter where a burrower at 0.10 takes
0.90. It is worse than the specialist at **both** foods it can reach, everywhere
it stands, which is exactly "a lower realised diet yield" — and A's finding 4
(`gamma = 1` makes breadth free in the *sum* while `theta = 0.2` makes the gate
abrupt) is the mechanism. **This campaign measured survival and lifetime, not
yield**: per-body intake by channel is workstream E's accumulator and is not in
these rows, so the yield step of that chain is inferred from the contract's
arithmetic rather than measured here.

**The confound, stated, and why it is not cancelled.** The bin-0.65–1.00
form-3 bodies are descendants whose diet mutated upward: of 75 entrants only 12
die and 63 are alive at the horizon. Later birth biases survival upward; the
lifetime mean over 12 deaths does not cancel selection into the lineage,
possible mutation at other loci, or habitat. The clean test is a matched
factorial, not another observational bin summary: clone the founder skimmer at
the same birth tick and locations with only `diet` changed (0.60 vs 0.85), then
cross the same fixed diet over forms, mutation and reproduction off, with E's
ledger reporting served, digestible, credited and billed energy by channel. A
birth-tick-matched whole-world hazard is a secondary check only.

## Accounting, and what was not measured

- **Conservation.** Worst `|mass residual|` over 108 rows **5.26e-10**; worst
  `|energy residual|` in arm 0 **5.12e-10** — both inside the contract's 1e-9
  tolerance and in the same band A measured.
- **Refusals and failures: none.** 0 of 108 rows `Invalid`, 0 `Failed`.
- **Censoring: 4 late windows are `null`**, the four worlds that ended empty.
  They are reported as lost worlds and excluded from the spatial and late means,
  and the exclusion is stated in the matrix table.
- **Net energy margin per body: not measured.** Workstream E's per-organism
  budget accumulator had not landed on `main` (checked at `b70624d` before the
  campaign was planned and again before it ran). Every row carries
  `movement.net_energy_margin_per_body: null` rather than a number derived from
  something else. What is reported instead is death cause, exact lifetime from
  `LifeEvent::Death { age_ticks }`, and the last-observed stores — at most 20
  ticks (1 s) before the death event, which is why they are called *last
  observed* and not *final*.

## What this does not establish

- **One price knob, at three levels.** Nothing here tests site fidelity, patch
  memory, territory, consumer density or available area. `move_cost` is the only
  thing that moved, which is the point of the design and also its limit: it shows
  that *this* knob buys range and depletion and not recovery, not that no spatial
  mechanism can.
- **Both raised levels overshoot for the grazer.** Every raised price kills the
  founder grazers before their first brood, and the gliders too except in
  `fast-leaf` at 0.0018, so the matrix contains no observation of a world with
  concentrated grazing *and* a breeding grazer. The response between 0.00036
  and 0.0018 is unsampled, and that is where an informative answer would be.
- **150 simulated minutes.** Recovery's rarity is a statement about this
  horizon: 13 crossings in 72 raised-price runs, mechanism unresolved. A's
  held-out stage showed 300 minutes changes nothing qualitatively for the
  control, but the raised-price arms were not run long.
- **Six training seeds, three matched apex arms, two configurations.** No
  held-out seed was touched, nothing was tuned, and no configuration is proposed.
- **The four extinctions are four events.** Three fell in arm 1 and one in arm 2,
  which is not enough to say anything about the apex.
- **No equation is proposed.** The pre-registered refutation branch was not
  reached — every cell is PARTIAL — so, per the brief, nothing here names a new
  mechanism or a new term.

## The next task this implies, named and not launched

**A finer price ladder between the shipped price and 0.0018, gated on the
founder grazer's first completed brood, with the glider reported separately.**
(Corrected after review: "first herbivore brood" is already satisfied at
`fast-leaf` 0.0018 by the glider, so the gate must name the grazer.) The
measured reason every cell is PARTIAL is that the cheapest raised price already
starves the grazer at about 180 seconds, before `bud_min_age_seconds` +
`gestation_seconds` and the reserve threshold can all be met, and recovery
stays rare for reasons this campaign did not record. The question the campaign was built to answer — does
concentrated grazing produce depletion *and* recovery — needs a world in which
concentrated grazers are still alive. The obvious matched arm is
`organism.move_cost` in {0.00036, 0.0006, 0.0009, 0.0012, 0.0018} on the same two
configurations, the same six training seeds and arm 0 only (the apex made no
measurable difference in A's screen or in this matrix), reporting founder-rig
first-brood counts beside the same spatial and crossing measures. That is 60
trials, about 3 wall minutes at this campaign's measured 58,729 ticks/s.

Three smaller measurements would make the result readable either way, and all
are cheap: **record each depleted cell's own `L*mu` and its foliage trajectory**;
**record post-depletion pressure** — time and stock at the last consumer visit,
visit count and served material after depletion, first recovery and
re-depletion times — so slow regrowth, returning consumers and intrinsically
marginal cells can be told apart; and **use workstream E's per-body ledger**
(landed on `main` at `46ac238`) so the skimmer's diet-yield step is measured
rather than inferred.

## Routine decisions made here, and their visible effect

| decision | effect |
| --- | --- |
| `organism.move_cost` **appended** to the parameter box as the fourteenth name, not inserted by axis | the first thirteen indices are exactly where A left them, so no candidate's vector moved and the default vector still builds the screen's world — confirmed by 36 matching state hashes |
| a row written before this name existed is **refused** by `from_bit_labels` on length, not padded | A's retained rows replay under the build that wrote them (`e635088`) and not under this one. Padding a missing component with a default would let a thirteen-component row silently become a fourteen-component one, and the `param_fingerprint` check would then be the only thing between a replay and a different world. The campaign's own reproduction check — re-running the matrix and comparing state hashes — is the stronger check and it passed |
| the price is a **matrix axis** (`--prices`), not four new declared candidates | a move-cost-only candidate would break the screen's "every candidate moves two or more names" rule, and a price crossed with the existing candidates keeps `baseline` and `fast-leaf` meaning exactly what they meant to A |
| a price outside the declared box is **refused before compute**, not clamped | the box is a declaration; running outside it silently would make the box prose |
| `movement.rs` declared with one line in `lib.rs` | a new file under `src/` is unreachable without its module declaration, exactly as a new subcommand is unreachable without its dispatch line; this is the only line outside the brief's named files |
| residence time estimated as `probes x 20 ticks`, the midpoint of `((n-1)*20, (n+1)*20)` | an occupancy shorter than one second is not resolved and reads as one second; at the measured 4–16 s residences the bias is a few per cent and changes no comparison |
| terminal stores taken at the last probe before death, not at death | at most 1 s stale; it is what a read-only observer of `World::step` can see, and the starvation/age separation (0.002 e against 1.36 e) shows the staleness does not blur the causes |
| the apex arms kept at 0/1/2 rather than dropped to arm 0 | A's rows are per arm, so keeping all three is what makes the 36-row state-hash reproduction check possible at all |
| whole-run rather than late-window crossings used in the `D` rule | a depletion in this world is absorbing, so a late-window count would report the stock of already-depleted cells rather than the flow of crossings |

## Compute and storage actually used

| | |
| --- | --- |
| Trials | 108, 0 skipped, 0 invalid, 0 failed |
| Ticks | 19.44 M declared; four worlds ended early |
| Wall | **320.4 s = 5.3 minutes** against the brief's 20-minute cap; 7.2 predicted |
| Throughput | 58,729 ticks/s on 8 workers |
| Workers | 8, the cap |
| Peak RSS | 21 MiB |
| Storage | `runs/ecology-v1-movement/` **1.6 MiB** against the 30 MiB budget; `runs/` is git-ignored, so none of it is committed |
| Background processes | none; nothing is left running |

Model usage: the harness exposes none and this session's is not visible to it.
The measured resource is the table above.

## Stop

The note and the commits are the deliverable. Nothing here changed an equation, a
§11 value, an ordering, the snapshot schema, the trainer, the display, the
running process, `state/`, or port 7393; no `cubarium-core` file and no file
under `crates/cubarium-search/src/es/` was modified; and no held-out seed was
touched.
