---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Workstream M — the plant budget of a depleted cell, measured

> **Retained 2026-09-18.** This is the cited plant-budget evidence. Links to
> round notes and briefs deleted with the flat-era material are Git history.

Evidence, not a decision. Nothing here changes §11, `producer.initial_fraction`
or any equation. The owner's options in §8 are stated, not chosen.

**Brief**: [`design/handoffs/ecology-v1-plant-budget-opus-2026-09-16.md`](../handoffs/ecology-v1-plant-budget-opus-2026-09-16.md),
step 3 of the reconciled next steps in the
[round-2 result](ecology-v1-round2-results-2026-09-16.md), from Astra's
[round-2 review](ecology-v1-round2-review-2026-09-16.md) (P2 on I, next steps
item 3).

**Build** `bdc2fcd` (`CUBARIUM_SEARCH_BUILD=bdc2fcd`), branch
`worktree-agent-a0d695b47426974f7`, on top of `1525604`.

## 1. The question, and why I's answer was not yet a measurement

Workstream I counted 1,355 depletion crossings across its 60 ladder runs and
found 971 of them in cells where no prey body was ever *observed* at a
one-second probe, reading that as cells seeded at `0.4·P_cap` (§11) that cannot
hold that foliage. Astra's P2: the visit was sampled every 20 ticks, the bites
were attributed to a sampled position rather than recorded where the stock
actually lost them, and `(L·μ)_crit` is a static proxy evaluated at a reference
nutrient with the reserve and the §4.4 reflush left out. Three proxies, one
conclusion. The correct next move was to measure the thing itself.

Astra's rule, which this campaign was designed against and which was fixed
before any of it ran:

> If the plant-only worlds cross at the same cells and times as the herbivore
> worlds and their measured plant budget is negative there, the over-seeding
> reading is confirmed. If the crossings disappear without herbivores, or the
> crossing cells' plant budget is positive, it is refuted.

## 2. What is now recorded, and where

**Core, opt-in, read-only.** `World::record_plant_budgets(true)`
(`crates/cubarium-core/src/world/budget.rs`, beside workstream E's body ledger)
opens a `PlantBudgetRecord` (`crates/cubarium-core/src/fields.rs`) in the plant
step's own scratch. Every term is booked **where §4 computes it**, accumulated
**every tick**, per cell:

| recorded | contract | site |
| --- | --- | --- |
| effective light `algae_light(W, L)`, `wet`, `drown`, `N⁻`, the Monod factor | §4.1 | `Fields::react`, subphase 3a |
| `P⁻`, `Q⁻`, `W⁻`, `P_cap = min(P_max, α·W⁻)` | §3.1, §4.2 | 3a |
| gross income `A` drawn out of `N` | §4.1 | 3a |
| maintenance paid from income, paid from reserve, unpaid | §4.3 | 3a |
| reserve share `q_share`, reserve refill | §4.4 | 3a |
| foliage grown from income; foliage grown from reserve (**the §4.4 emergency reflush**, the `Q → P` transfer); wood grown | §4.4 | 3a |
| senescence | §4.5 | 3b |
| ripening | 3c | 3c |
| dieback `κ·unpaid`; the dying stand's foliage, reserve and wood | §4.6, §4.7 | 3d |
| propagule reserve sent; starter wood, foliage and reserve landed | §4.8 | 3h |
| **exact consumer withdrawal** from `P`, `F`, `D` and `C` | §6.4 | `world::step`, at `fields.p[cell] -= q` and its three siblings |

The withdrawal is booked in the cell the mouth is standing in, at the statement
the stock loses the bite. It is not attributed from a sampled position and
cannot miss a bite taken between probes — the two things Astra's P2 named.

**Scope note.** The brief expected the plant step in
`crates/cubarium-core/src/world/step.rs`. §4.1–4.8 are actually in
`crates/cubarium-core/src/fields.rs::react`, which the tick calls from
`step.rs`. Reproducing that arithmetic in `step.rs` would have made the record
a second implementation of the contract's hardest equations, which is the class
of proxy this workstream exists to remove; so `fields.rs` carries the record's
writes instead. They are additive, guarded by one `Option` check, and no other
worker owns that file. `world/budget.rs` carries the `World` API, which is the
contingency the brief allowed ("beside the body ledger") — it also keeps the
record out of `World`'s field list and out of `world/mod.rs`. `world/step.rs`
carries only the four withdrawal hooks, each three lines, away from the hunter
strike region another worker owns.

### The record is inert, and it closes

Tests first, from the brief's definitions
(`crates/cubarium-core/tests/plant_budget.rs`, 9 tests):

- **Inert.** 9,000 ticks of an ordinary seed-4,242 world, three ways — nobody
  recording, recording untouched, and recording read *and* identity-checked on
  every single tick — end on the identical `state_hash`, the identical
  `ecology_hash` and byte-identical `encode_snapshot`. The record is never
  persisted, never hashed, never read back by the tick.
- **It closes.** Per cell, to `1e-9`:
  `ΔP = (income-fed growth + reflush + propagule) − (senescence + ripening +
  death) − withdrawal`, and the matching `Q` and `W` identities. Recomputed in
  the test from the record's published terms, not from the implementation's own
  residual helper. Measured worst residual over 9,000 ticks: **3.5e-15**.
- **Every branch is exercised.** At the shipped defaults nothing goes unpaid,
  no stand dies, no reflush opens and no propagule lands in 9,000 ticks — so
  those branches are covered by two further fixtures (wood maintenance ×20 with
  a third of the cells opened bare of foliage; a cheap-propagule world with a
  third of the cells opened bare of wood) which reach death, dieback, unpaid
  maintenance, the reflush and both sides of the propagule transfer, and close
  the identity at **1.7e-15** and **3.0e-15**.
- **The withdrawal is exact.** Summed over cells it equals the world's own
  `IntakeDiagnostics` counters — `producer_eaten`, `fruit_eaten`,
  `litter_eaten`, `carrion_eaten` — which are written at the same sites into
  different variables and are not derived from the record.
- **The plant-only world is a configuration.** With `founders.kinds` empty and
  `founders.count` zero (`world/lifecycle.rs:52-63`) no animal is placed, the
  population is 0 at tick 0 and at tick 9,000, every withdrawal counter is
  exactly zero, `request_ticks` is zero, and the identities still close. Verified
  as the brief asked.

### The counter split

`crates/cubarium-search/src/plant_budget.rs`, 12 tests on hand-built sequences
(`crates/cubarium-search/tests/plant_budget_measures.rs`). Every crossing the
existing `CrossingCounter` reports — the same counter, through the same closure
that drives I's per-cell record, so the three cannot drift — is classified:

- **crossed with exact withdrawal**: the cell's cumulative exact withdrawal has
  risen by more than `1e-9` since the cell last recovered, or since the record
  opened for a cell that never recovered;
- **crossed without**.

I's probe-based "ever visited" is carried unchanged beside it, never instead of
it. Each crossing also carries the measured plant budget `in − out` over the
6,000 ticks before it (from the newest 600-tick boundary at or before
`tick − 6,000`, with the window's actual length reported, since a crossing does
not have to fall on a boundary), the exact withdrawal over that same window
reported *beside* the budget and never folded into it, the gross income, the
unpaid maintenance, the death-dropped foliage, and the mean effective light,
nutrient and wood the plant step actually used over the ticks the cell was
alive — absent, not zero, when it was alive for none of them.

## 3. The campaign

```bash
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test --release -p cubarium-core    # 511 passed, 4 ignored
CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target cargo test --release -p cubarium-search  # 188 passed, 1 ignored
CUBARIUM_SEARCH_BUILD=bdc2fcd CARGO_TARGET_DIR=/home/wrysk/wryskware/cubarium/target \
  cargo build --release -p cubarium-search

# the control: I's arm-0 rows at the shipped price, on this build, record off
cubarium-search calibrate --stage present-off --candidates baseline,fast-leaf \
  --seed-set training --seeds 6 --arms 0 --prices 0.00036 --ledger \
  --ticks 180000 --sample-every 600 --introduce-tick 6000 \
  --workers 8 --wall-seconds 480 --out runs/ecology-v1-plant-budget

# arm 1 — herbivores present, record on
cubarium-search calibrate --stage present     ... --ledger --plant-record ...

# arm 2 — herbivores absent, record on
cubarium-search calibrate --stage absent      ... --ledger --plant-record --no-animals ...
```

36 runs of 180,000 ticks: **54.4 s + 54.9 s + 39.9 s = 2.5 wall minutes** on 8
workers, 0 skipped at the cap, 0 invalid, 0 failed, peak RSS 19/37/30 MiB.
`runs/ecology-v1-plant-budget/` is **13 MiB**: the three `evals.jsonl`, each
row carrying the crossing rows plus a coarse all-cell summary (whole-run
aggregates for all 1,280 cells, no time resolution — that is what lets a cell
that crossed in one arm be looked up in the other, which the overlap number is
made of). No per-tick series is kept.

### The reproduction checks, before anything was interpreted

1. **This build does not move the world.** With the record **off**, all 12
   `(configuration, seed)` rows carry workstream I's retained
   `final_state_hash` *and* `final_ecology_hash` at `move_cost = 0.00036`,
   arm 0: **12 of 12**. The core edits are inert by themselves.
2. **The record does not move the world.** With the record **on**, the same 12
   rows carry the same hashes: **12 of 12**. More than that, the record-off and
   record-on rows are identical in the **whole `metrics` block** (12/12) and in
   `movement.crossings`, `movement.census`, `movement.late`,
   `movement.whole_run`, `movement.founder_broods`, `movement.margins` and
   `movement.depletion` — I's own per-cell record — **12/12 each**. The
   preflight pair at 20,000 ticks also reproduces I's own preflight hash
   `4692485605019165535` exactly, on and off.
3. **The accounting closes in the campaign, not only in the tests.** The
   record's own per-cell identity residual at the horizon, over 180,000 ticks:
   worst **8.9e-12** across the 12 present-arm runs and **1.2e-11** across the
   12 absent-arm runs, against a 1e-9 acceptance.
4. **The counter split is driven by the same counter.** Per run, the split's
   `total_crossings` equals `movement.crossings.depletions` — the aggregate
   number A, F and I all report — in **24 of 24** instrumented runs, and
   `crossings_dropped` is 0 in all 24 (cap 4,096, most in any run 75).
5. **The plant-only arm really is plant-only.** Population 0 in 12/12 runs,
   all 180,000 ticks run (the empty-world stop is disabled for this arm by the
   flag, not by an exception the ordinary path could take), and the recorded
   consumer withdrawal is **exactly 0.0** in every cell of every run.

### The record's cost

| | mean run, 180,000 ticks | |
| --- | --- | --- |
| record off | 27,089 ms | |
| record on | 27,457 ms | **+1.36 %** |

Same 12 worlds, same binary, same host, 8 workers. Under the brief's 3 %. Off,
the record allocates nothing and costs one `Option` check per cell per
subphase: the record-off stage's throughput, 39,729 ticks/s, is within noise of
I's ladder at 39,910–54,134 ticks/s for the same work (I's 54,134 figure is its
whole 60-run ladder, which includes cheaper shorter-lived worlds at higher
prices; the like-for-like comparison is this campaign's own off-stage against
its on-stage). A row grows from 26 KiB to 488 KiB because of the all-cell
summary, not because of the tick loop.

## 4. Arm 1 — herbivores present

Six training seeds × two configurations, the shipped price, arm 0, 180,000
ticks. **73 crossings in 12 runs**, in 73 distinct cells.

| configuration | crossings | cells | **with exact withdrawal** | **without** | without *and* negative budget | I's probe: "visited" | I's probe: "never" |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 68 | 68 | **0** | **68** | **68** | 41 | 27 |
| fast-leaf | 5 | 5 | **0** | **5** | **5** | 1 | 4 |

Not one of the 73 crossings had a single metre of foliage withdrawn from its
cell since that cell last recovered. Stronger, and measured over the whole
horizon rather than since the last recovery: **0 of 73 crossing cells lost a
metre of foliage to any mouth in 180,000 ticks** — while the same runs withdrew
**1,511 m** (baseline) and **2,380 m** (fast-leaf) of foliage per run, from
663–1,113 *other* cells each. Grazing in these worlds is heavy, and it never
touches the cells that deplete.

**I's probe-based split is wrong in the direction opposite to the one Astra
worried about.** Astra's concern was that a sampled visit misses grazing
between probes, so "never visited" would understate consumption. On these
crossings the flag *over*-states it: the probe called 41 of the 68 baseline
crossings "visited", and the exact counter finds withdrawal in none of them. A
prey body stood in the cell and took nothing out of it, which is what a §6.4
type-II mouth does in a cell holding a quarter of an already small stock.

What the crossings look like, over the 6,000 ticks before each one:

| arm | configuration | n | median net budget | negative | median withdrawal | median gross income | median unpaid maint. | stand died | median effective light | median `N` | median `W` |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| present | baseline | 68 | **−0.00236** | 68 | 0.0000 | 0.00330 | 0.00009 | 2 | 0.212 | 0.673 | 0.0397 |
| present | fast-leaf | 5 | **−0.00143** | 5 | 0.0000 | 0.00126 | 0.00000 | 0 | 0.110 | 0.913 | 0.0232 |
| absent | baseline | 382 | **−0.00566** | 382 | 0.0000 | 0.00506 | 0.00034 | 2 | 0.913 | 0.122 | 0.0826 |
| absent | fast-leaf | 180 | **−0.00436** | 180 | 0.0000 | 0.00430 | 0.00000 | 0 | 0.972 | 0.108 | 0.0814 |

Every crossing in both arms has a negative measured plant budget over the window
before it. Only 2 of the 73 present-arm crossings were in a cell where a stand
died; the other 71 are stands that thinned below a quarter of their opening
foliage while still alive. Unpaid maintenance is present but small, so this is
not primarily a maintenance default — it is a stand whose income cannot replace
what senescence takes.

## 5. Arm 2 — herbivores absent

The same six seeds and two configurations with `founders.kinds` empty and
`founders.count` zero. **562 crossings in 12 runs.**

| configuration | present arm | absent arm | ratio |
| --- | ---: | ---: | ---: |
| baseline | 68 | **382** | 5.6× |
| fast-leaf | 5 | **180** | 36× |

The crossings do not disappear without herbivores. They multiply.

**Overlap of crossing cells, per seed** — the number Astra's rule turns on:

| configuration | seed | present | absent | in both | present-only | absent-only |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 1001 | 14 | 75 | **14** | **0** | 61 |
| baseline | 1002 | 3 | 50 | **3** | **0** | 47 |
| baseline | 1003 | 10 | 73 | **10** | **0** | 63 |
| baseline | 1004 | 3 | 69 | **3** | **0** | 66 |
| baseline | 1005 | 5 | 62 | **5** | **0** | 57 |
| baseline | 1006 | 33 | 53 | **33** | **0** | 20 |
| fast-leaf | 1001 | 2 | 36 | **2** | **0** | 34 |
| fast-leaf | 1002 | 0 | 31 | — | **0** | 31 |
| fast-leaf | 1003 | 0 | 36 | — | **0** | 36 |
| fast-leaf | 1004 | 0 | 22 | — | **0** | 22 |
| fast-leaf | 1005 | 0 | 28 | — | **0** | 28 |
| fast-leaf | 1006 | 3 | 27 | **3** | **0** | 24 |

Every cell that crosses with herbivores also crosses without them, in every one
of the twelve worlds: **73 of 73, with zero present-only cells in any seed**.
And it crosses *sooner* without them: for all 73 paired cells the absent arm's
first crossing is at or before the present arm's, median gap 9,530 ticks
(baseline) and 9,120 (fast-leaf).

| configuration | present-arm crossing cells | ever grazed (whole run) | negative budget, present arm | negative budget, absent arm | also crossed in the absent arm | absent arm crossed first |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| baseline | 68 | **0** | **68** | **68** | **68** | 68 |
| fast-leaf | 5 | **0** | **5** | **5** | **5** | 5 |

## 6. The verdict

**Confirmed, by Astra's rule, on both branches of the test and in both
configurations.**

- The plant-only worlds cross **at the same cells**: 73/73 overlap, 0
  present-only cells in any of the twelve worlds.
- **At the same or earlier times**: 73/73 absent-arm crossings are at or before
  the paired present-arm crossing, median gap ≈ 9,500 ticks.
- **With a negative measured plant budget there**: 73/73 in the present arm and
  73/73 in the absent arm, over the 6,000 ticks before the crossing and over
  the whole run.
- The refutation branches did not fire: the crossings did not disappear (they
  rose 5.6× and 36×), and no crossing cell's plant budget was positive.

The extra evidence the exact withdrawal adds, which the rule did not ask for
and which is the strongest single number here: **0 of 73 crossing cells lost a
metre of foliage to a mouth in 180,000 ticks**, in worlds that withdrew 1,511
and 2,380 m of foliage per run from a thousand other cells. "A depletion event
is almost never a grazed-out cell" is no longer a hypothesis resting on sampled
visits; at this price and these two configurations it is a measurement, and the
measurement is stronger than the hypothesis was.

**What the crossing cells are.** In the absent arm, comparing the 382 baseline
crossing cells against the 6,290 other watched cells (medians):

| group | n | `P₀` | static `L·μ` | effective light | `N` | `W` | gross income (whole run) | unpaid maintenance | negative budget |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| crossing | 382 | **0.0653** | **0.272** | 0.920 | 0.195 | **0.079** | **0.445** | 0.0130 | 382/382 |
| other | 6,290 | 0.1014 | 0.423 | 0.748 | 0.258 | 0.295 | 6.059 | 0.0000 | 590/6,290 |

A cell that crosses is a dry, nutrient-poor, thin-wooded cell that draws about
one fourteenth of the gross income its neighbours do. Its static `L·μ` is low
because its *moisture* is low, not its light — its effective light is if
anything higher. I's `(L·μ)_crit` screen was therefore picking out roughly the
right cells for roughly the right reason; it just could not show that it was.

**A negative budget is necessary but not sufficient.** In the absent arm 162
watched cells per run (baseline) and 105 (fast-leaf) carry a negative whole-run
plant budget, of which 64 and 30 actually fall below a quarter of their opening
foliage. The rest decline without crossing the threshold.

## 7. What this does not establish

- **The two arms are not the same world without grazing.** Removing the
  founders removes their feces, carcasses and respiration, so the absent arm is
  a different nutrient regime, not a grazing-free copy: median `N` at the
  crossing cells is 0.12–0.19 in the absent arm against 0.67–0.91 in the
  present arm. That is inherent to the matched design Astra specified and is why
  the *confirmation* is not made to rest on it: the 73 present-arm crossings
  already have zero exact withdrawal and a negative measured budget on their
  own, in the herbivore-present world. The absent arm's role is to show the
  crossings do not disappear, and it shows that strongly.
- **Not an equilibrium.** `P` at tick 180,000 in a plant-only world is the state
  the plant step reaches by the horizon, not a proven fixed point. §8's option-B
  numbers use it as a measured stand-in and say so.
- **One price, two configurations, arm 0, six training seeds.** Nothing here
  speaks to the other four ladder prices, to the held-out seeds, to arms 1 and
  2, or to any configuration outside `baseline` and `fast-leaf`.
- **The 73 present-arm crossings are a small sample**, and 33 of the 68
  baseline ones are from one seed (1006). The absent arm's 562 are not small,
  but they are the arm with the altered nutrient regime.
- **No universal critical `L·μ` is claimed, defined or used.** The static `L·μ`
  appears in the rows for comparison with I only. Astra's instruction not to put
  a critical `L·μ` into anything is followed.
- **Nothing about the animals' failure to feed.** That the crossing cells are
  never grazed says the depletion counter is not a grazing signal. It says
  nothing about whether the controller feeds well elsewhere — that is H's and
  the score work's question.
- **The "since the last recovery" clause is barely exercised by this campaign.**
  There were **2 recoveries in all 24 instrumented runs**, so for almost every
  crossing the withdrawal interval is the whole run to that point and the clause
  reduces to "since the record opened". The clause itself is tested on hand-built
  sequences (`the_withdrawal_clock_restarts_at_every_recovery`), not here.
- **The record measures material, not energy.** Energy is booked by the existing
  `FieldLedger` and is untouched.

## 8. What the owner would be deciding — stated, not decided

Both options are §11 changes and belong to Wrysk. The measurement now exists to
choose between them; this note does not choose.

**Option A — a plant-only warm-up before founding animals.** Run the world with
no animals for a fixed number of ticks, then found the roster. The cells that
cannot hold their seeded foliage lose it before anything is watching, and the
first tick the display shows is the plants' own state.

*One number*: in the plant-only arm the first crossing is at tick **42,520**
(baseline) and **54,980** (fast-leaf), the median at **141,220** and
**147,060**, and 271 of 382 (and 144 of 180) crossings are still arriving after
tick 120,000. A warm-up long enough to absorb the decline is of the order of the
whole 180,000-tick horizon — 150 simulated minutes — not a few thousand ticks.
That is the cost of this option, and it is the number that decides whether it is
an option at all.

*Visible consequence*: the display opens on a world that has already sorted
itself. Dim, dry cells are thin from the first frame; the bright wet ones are
full. Nothing greens uniformly and then fades, because the fade has already
happened. The cost is that the first animals meet a world with 3.6–4.2× the
seeded standing foliage (§7 caveat: measured at the horizon, not proven
stationary), which is a different founding ecology from the one every result so
far was measured in.

**Option B — seed each cell below its own measured terminal state** (renamed
after Astra's round-3 review: the tick-180,000 plant-only state is not a proven
fixed point, its predictors — effective light, moisture, `N`, `W` — are
endogenous and spatially coupled, and changing the opening `P` changes the
later state being fitted, so a terminal regression is an initialiser, and
"seeded at it, it stays there" is a new hypothesis, not a consequence of this
measurement). Replace the single `initial_fraction · P_cap` with a per-cell
opening value derived from a fit to the plant-only endpoint. Astra's
alternative, which keeps the coupled equations instead of a fitted proxy: a
deterministic **whole-field plant-only preconditioning** — initialise the full
§11 field, advance the ordinary §4 dynamics with animals absent for a declared
seed- and config-bound duration or until declared moving-window criteria are
met, then found the animals from that joint state, and call the result a
fixed-age preconditioned opening, not an equilibrium. That is option A with a
declared procedure.

*One number*: the opening foliage distribution over the 1,112 watched cells.

| series | `ΣP` | median | p10 | p90 | CV |
| --- | ---: | ---: | ---: | ---: | ---: |
| seeded at `0.4·P_cap` (today) | 107.15 | 0.0987 | 0.0420 | 0.1441 | **0.389** |
| plant-only at tick 180,000, baseline | 389.37 | 0.4478 | 0.0435 | 0.5265 | **0.528** |
| plant-only at tick 180,000, fast-leaf | 447.34 | 0.4624 | 0.0754 | 0.5714 | **0.455** |

Today's seeding puts **14.6 %** (baseline) and **9.4 %** (fast-leaf) of watched
cells *above* what the plant step alone sustains, and the cells that cross are
seeded at about **25 times** what they end up holding: median `P₀` 0.0653
against median `P` at the horizon of 0.0024, a ratio of **0.039**.

*Visible consequence*: dim and dry cells begin visibly less lush than bright wet
ones — the opening picture is heterogeneous rather than uniform, with the
coefficient of variation of opening foliage rising from 0.389 to roughly
0.45–0.53 — and the uniform green-then-fade of the first two simulated hours
stops happening, because nothing is seeded into a deficit. The same numbers say
the world would open *greener overall*, not thinner: total opening foliage would
rise roughly 3.6–4.2×, which is a presentation decision as much as an ecological
one.

**Not an option this measurement supports**: leaving §11 as it is and reading
the depletion counter as a grazing signal. At this price and in these two
configurations the counter measures the seeding, not the herbivores.

## 9. Next task, named and not launched

**A whole-field plant-only preconditioning comparison, not a per-cell fit**
(revised after Astra's round-3 review; the first version proposed fitting the
tick-180,000 `P` per cell and calling it a local equilibrium, which the option
text above now withdraws). Save plant-only whole-field states at several ages
spanning the first crossing through 180,000 ticks on the six training seeds
with the record on; measure moving-window changes in total and per-cell `P`,
`W`, `Q`, `N`, exact plant income and loss, threshold crossings and spatial
variance; found identical rosters into status quo and a small number of those
states; and give Wrysk the opening frames and the early founder outcomes. That
is the existing `--no-animals --plant-record` path plus a state save and a
founding, and it is what turns this evidence into a §11 proposal (option A
with a declared, deterministic, seed- and config-bound procedure) that Wrysk
can accept or refuse. A per-cell endpoint fit, if ever used, is an initialiser
and not an equilibrium rule.

Beside it, and independent of the seeding decision: **the depletion counter
should be re-read everywhere it has been quoted.** A, F and I's depletion and
recovery counts are measurements of the seeding, not of grazing pressure, at
least at the shipped price. Nothing needs re-running — the counts are correct —
but the sentences around them do.

## Decisions this workstream made, and why

| decision | why |
| --- | --- |
| the record's writes live in `fields.rs`, not `world/step.rs` | §4.1–4.8 are computed there; writing them in `step.rs` would mean a second implementation of the contract's hardest equations, which is exactly the class of proxy Astra's P2 rejected |
| the `World` API lives in `world/budget.rs` | the brief's stated contingency; it also keeps the record out of `World`'s field list and out of `world/mod.rs`, which nobody owns cleanly |
| the record hangs off `EcoScratch`, not off `World` | `EcoScratch` is already the plant step's own transient, already never persisted or hashed, and already reachable from both `react` and the §6.4 withdrawal site — so no new `World` field and no `world/mod.rs` edit |
| accumulated **every tick**, cumulative, never a per-tick series | a per-tick series over 1,280 cells × 180,000 ticks is not storable; cumulative counters differenced at 600-tick boundaries give every window the analysis needs at 1.4 % of run time |
| the search side keeps a ring of eleven 600-tick boundary snapshots | that is exactly the 6,000-tick window Astra asked for, and no more; a crossing off a boundary reports its own window length rather than pretending |
| the identity is checked cumulatively per run, not per tick, in production | the per-tick check is in the core test over 9,000 ticks; the per-run check is O(cells) and catches the same failure |
| `--no-animals` is a `RunOptions` flag that empties the founder roster, not a new command | it keeps the herbivore-present arm on byte-identical code to I's ladder, which is what makes the 12/12 hash reproduction meaningful |
| the empty-world stop is disabled only for `--no-animals` | a world with no population by construction has not collapsed; every other run keeps the honest stop |
| the two arms ran through `calibrate`, not a new runner | the present arm is then literally I's command plus one flag, so "the record moved nothing" is tested against I's own retained rows rather than against a fresh harness |
| `I`'s "ever visited" is kept in every row | the two splits disagree on 42 of 73 crossings; that disagreement is a result, and it would be invisible if the old field had been replaced |
| a third stage with the record **off** | it separates "the flag moved the world" from "this build moved the world"; without it a 12/12 hash match with the record on would not distinguish the two |
| the all-cell summary is whole-run aggregates for all 1,280 cells | the overlap and the cross-arm budget lookups need a cell that crossed in *one* arm to be present in the *other*; a crossing-cells-only file cannot answer Astra's question |
