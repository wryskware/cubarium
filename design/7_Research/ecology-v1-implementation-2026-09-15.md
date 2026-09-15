---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Ecology v1 — implementation result

Evidence, not a decision. This records what was built against
[the ecology v1 contract](../ecology-v1-contract.md) §3–§10 under the
[Opus handoff](../handoffs/ecology-v1-opus-2026-09-15.md), what the suite says about it, and
what the §13.2 scenarios measured. Nothing here promotes anything to canon, and nothing here
was tuned: every §11 value is the contract's, and every number below is what the simulator
produced at those values.

**Three runs are recorded.** Run 1 implemented §4.4 as it stood and measured finding B0-1 —
the plant reserve could never persist. Fable fixed that in the contract (`3316980`), and
**repair cycle 1** implemented the revision and re-measured as run 2. Astra's
[implementation review](ecology-v1-implementation-review-2026-09-15.md) then found one
implementation P1 (3h read the wrong recipient state), one contract P1 (`ecology_hash` was
blind to every ecology v1 pool), four scenarios that did not measure their stated claims, tests
that could pass a wrong implementation, and — finding 5 — **this note reporting censored and
contrary measurements as passes**. **Repair cycle 2** fixes all of that and re-measures as run
3, which is the current measurement. Runs 1 and 2 are kept below, unchanged, so each repair's
effect is visible rather than described.

Run 3's section restates every scenario against the **literal §13.2 criterion** and marks a
result unresolved wherever the measurement is censored, missing or contrary — which is most of
them.

## Build and commit

- Baseline `4283040` on `main`. Run 1 is the code commit `b1dd394` and the note `6d304f1`.
- **Repair cycle 1** follows contract commit `3316980` (§4.4 rewritten after run 1's finding
  B0-1; §11 gains `q_share` 0.2 and `p_reflush` 0.25; §14 gains `reserve_share` and
  `reflush_below`; B4a's expectation corrected; all eleven of run 1's interpretations accepted,
  §18 third round). Implemented as `26c382e`.
- **Repair cycle 2** follows commit `5a0c813`: Astra's
  [implementation review](ecology-v1-implementation-review-2026-09-15.md), §15.1's new
  care-masked `ecology_hash`, the rewritten §13.2 rows for B0, B1b, B3, B4b, B6b and B7, and
  §18's fourth round.
- **Repair cycle 3** follows commit `88fe2ac`: Astra verified cycle 2 and accepted six of its
  seven items. The one left was B6 — its upkeep was a reconstruction that omitted the
  rotational motor charge and every bill paid by a body that died in the tick, so the reported
  ratio was an upper bound rather than the actual one §13.2 B6b requires. One item, and the
  last cycle.
- `graft build` refreshed after each change.
- Schema 16, config version 8. No migration path exists anywhere in the tree.

## Verification (after repair cycle 2)

The four commands Astra's review required, run in this workspace on 2026-09-15 — the sandbox
that blocked them was Codex's, not the repository's (review finding 9):

```bash
cargo test -p cubarium-core      # 466 passed, 0 failed, 2 ignored
cargo test -p cubarium-search    #  68 passed, 0 failed
cargo test -p cubarium           # 569 passed, 0 failed, 16 ignored
cargo run -p cubarium-core --release --example ecology_v1_scenarios -- all   # 71 s wall, exit 0
```

Repair cycle 1's totals, for comparison: core 461, search 66, host 569; repair cycle 2's: core
466. The six new core tests are the A1 stock-by-stock isolation arms, the A4 four-food ledger,
the A7b `ecology_hash` perturbation test, the two A9 propagule regressions, and repair cycle 3's
body-bill completeness test; the two new search tests are the schema-16 parameter check and the
exclusion-list check.

Repair cycle 3 re-ran **B6a and B6b only**, as its brief directs; every other scenario figure
below is from the full run-3 pass and is unaffected, because nothing outside B6's reporting
changed.

All three suites are green. The two ignored core tests and the sixteen ignored host tests are
pre-existing (`#[ignore]` fixture regenerators and capture-writing art studies), untouched here.

The accounting tests A1–A9 are `crates/cubarium-core/tests/ecology_v1.rs`, now with three more
A1 arms and a three-band A2b fixture covering the repaired §4.4 branches (a share taken, a full
reserve taking none, a reflush gated and capped), plus one dedicated unit test,
`the_reflush_threshold_and_the_reserve_share_are_the_repaired_allocation`, which proves that a
stand at or above `p_reflush · P_cap` never draws its reserve for foliage. A green suite is
necessary and not sufficient; the review re-derives A1, A2a, A3b, A6 and A9 independently.

## Run 3 — every scenario against the literal §13.2 criterion

This is the current measurement, after repair cycle 2. **Every row states the contract's own
criterion and then says whether the measurement meets it**; where it is censored, missing or
contrary, the verdict is *unresolved*, not a pass. Astra's review named five results this note
had overstated — B1a, B3, B4b, B5's skimmer and B6a — and those are the first five to read.

Fixtures changed in this cycle: B0 holds propagules off (a B0x arm reports the export), B1b
reports foliage apart from total plant material, B6 measures the **complete** bill the world
booked — maintenance, sensing and both halves of the motor charge, over every body billed
including any removed later in the same tick — and the escrow debit behind every birth; and B7
pins the donor's and the recipient's light per cell so only the recipient changes between its
arms. **B6's figures were corrected again in repair cycle 3**, after Astra's verification found
the cycle-2 reconstruction was a lower bound on upkeep; the section below carries the corrected
numbers and says what moved.

### B0 — stand baseline, propagules off

| class | P | W | Q | Q_max | F | dP/dt | dW/dt |
| --- | --- | --- | --- | --- | --- | --- | --- |
| average | 0.0977 | 0.1050 | 0.0525 | 0.0525 (full) | 0.0000 | 2.5e-5 | 0.0 |
| bright | **0.4789** | **0.3934** | **0.1967** | 0.1967 (full) | 0.0090 | 1.8e-4 | 2.2e-4 |

B0x, the same fixture with §4.8 on: bright P 0.4695, W 0.3578, Q 0.0894, 0.1660 m exported, 0
cells established. Average is identical in both arms — it never clears the donor threshold.

**Criterion (§13.2):** "approaches the §11 table — average `P ≈ 0.5, W ≈ 0.33`; bright
`P ≈ 0.56, W = 0.6`."
**Verdict: unresolved.** Bright is 14 % below the hand `P` and 34 % below the hand `W`; average
is 80 % below its hand `P`. Both are still moving at the horizon, so neither is an equilibrium
and the comparison is against a trajectory, not a steady state.

**What the repair changed, and what R2-1 really was.** Run 2 reported bright `Q = 0.0894` and a
widening gap from the hand table, and this note called that evidence of the reserve share's
cost. Astra was right that it was a **scenario-design defect**: with propagules off, the same
stand holds `Q = 0.1967` — its reserve exactly full — and `W` is 0.3934 rather than 0.3578. The
0.1660 m the stand exported in run 2 is the whole difference. R2-1 stands as a finding only in
its reduced form: `q_share` does cost growth at a 30-minute horizon (bright `W` 0.393 here
against run 1's 0.488), but a third of what run 2 reported was the export, not the share.

### B1a — one pinned grazer, one bright mature stand, 12,000 ticks

| measure | run 3 |
| --- | --- |
| leaf / fruit bitten | 0.6014 / 0.0076 m over the 249 s it lived |
| mean bite rate | 2.45e-3 m/s = 3.8× §11's sustainable bright yield |
| reserve-saturated ticks | **0 of 4,165** |
| stand at horizon | P 0.0007, W 0.3627, Q 0.0000, Wd 0.0296 |
| stand death | **censored at 36,000** |
| grazer starved | 4,976 (249 s) |

**Criterion (§13.2):** "bite exceeds yield ~40×; **reserve saturates first**; the stand is
stripped, reflushes from `Q`, and **dies**; the grazer then starves."
**Verdict: unresolved on two of four clauses.** The stripping, the reflush and the starvation
all happen. But the reserve **never** saturates — not once in 4,165 requesting ticks — so the
ordering the criterion names is not established; and the stand does **not** die inside the
horizon, so "and dies" is censored rather than observed. The bite is 3.8× the sustainable yield,
not ~40×: §11's 40× is the bite at `P = 0.5` with a full mouth, and the realised mean includes
the collapse, during which the type-II term throttles the mouth. Run 2 called this
"directionally unchanged"; that wording was wrong, and this is the correction.

### B1b — one mobile legacy grazer on a 5 × 5 mature region

| class | cells visited | **foliage ΣP** | total plant Σ(P+W+Q+F) | min stand P | stand deaths | grazer |
| --- | --- | --- | --- | --- | --- | --- |
| bright | 61 | **11.972 → 0.0197** | 26.950 → 9.150 | 0.0004 | 17 | starved 25,239 (1,262 s) |
| average | 44 | **2.443 → 0.0460** | 6.381 → 2.282 | 0.0004 | 0 | starved 9,313 (466 s) |

**Criterion (§13.2):** bright — "coexistence with retained foliage is **expected**"; average —
"the grazer is expected to run the region down"; "a bright failure is a finding."
**Verdict: bright unresolved (a failure, which the contract says to report); average met.** The
foliage figures are now foliage: run 2's "ΣP painted 23.037" was in fact total plant material,
2.25× the 11.972 m of foliage actually painted (Astra's finding 3). Correcting the label does
not change the outcome — the bright region still ends with 0.16 % of its foliage and 17 of 25
stands dead. What it does change is the size of the claim: the grazer removed 22.7 m of leaf
from a region that opened with 12.0 m, so it ate roughly twice the standing crop over the run,
which is a statement about turnover, not about a 23 m larder.

### B2 — depletion and relocation, three bright stands three cells apart

| stand | ticks on | min P | Q at departure | P at horizon | Q at horizon |
| --- | --- | --- | --- | --- | --- |
| (5,8) | 867 | 0.0004 | 0.0249 | 0.0004 | 0.0000 |
| (8,8) | 1,024 | 0.0003 | 0.0000 | 0.0003 | 0.0000 |
| (11,8) | 814 | 0.0009 | 0.0564 | 0.0009 | 0.0000 |

42 cells visited; grazer starved at 7,464 (373 s).
**Criterion (§13.2):** "the grazer leaves a stand near the type-II floor and moves on; whether a
stand recovers before return is reported, not assumed."
**Verdict: unresolved, and untestable in this fixture.** The grazer does not leave near the
type-II floor `K_P = 0.45` — it strips each stand to `P ≈ 0.0004`, three orders below it — and
it dies before returning to any of them, so recovery-before-return is never exercised. Astra
classified this as a scenario-design defect (finding 8, item 4) and it remains one: a fixture in
which the forager dies first cannot answer the question the row asks.

### B3 — plant recovery after defoliation

| arm | 0.5·P* | 0.9·P* | reserve dip → end |
| --- | --- | --- | --- |
| bright, B0-measured | **16,465 (823 s)** | **31,257 (1,563 s)** | 0.1967 → 0.0000 → 0.1018 |
| bright, §11 hand | 8,626 (431 s) | 25,755 (1,288 s) | 0.3000 → 0.0000 → 0.1500 |
| average, B0-measured | censored | censored | 0.0525 → 0.0000 |
| average, §11 hand | censored | censored | 0.1600 → 0.0000 |

**Criterion (§13.2, corrected in this round):** "reflush from `Q` first (a visible `Q` fall),
then income-limited; the §11 '~10 min' for bright `0.9·P*` is a hand estimate that ignores
ripening's drain above `P = 0.45`, so **the measured time stands and is not a pass/fail**;
average is expected to sit near breakeven and be reported censored."
**Verdict: bright met, average met.** The visible `Q` fall and refill is there in both bright
arms, and with the corrected criterion the 1,563 s and 1,288 s are measurements rather than
misses. Average is censored in both arms, which is what the row expects. Run 2 called B3-1
"resolved" while its 0.9 arm was censored; with propagules off the B0-measured bright arm now
reaches 0.9·P* inside the horizon, so the claim is now actually supported.

### B4a — three pinned grazers on one isolated bright mature stand

| measure | value |
| --- | --- |
| `Q` reached zero | 3,174 (159 s) |
| dieback opened | 3,175 (159 s) |
| stand death | **censored, as §13.2 now expects** |
| `W` decline since dieback | 0.3935 → 0.2854 m over 1,641 s |
| measured e-folding time | **5,110 s (85 min)** against `1/(κ·m_w)` = 5,000 s (83 min) |
| dead wood at horizon | 0.0914 m |
| grazers starved | 8,175 (409 s) ×3 |

**Criterion (§13.2, corrected in repair cycle 1):** "`Q → 0`, dieback opens, `W` declines at
`κ·m_w` (e-fold ~80 min, so death is expected censored), no regrowth, grazers starve."
**Verdict: met, every clause.** The measured e-fold is within 2.2 % of `κ·m_w`.

### B4b — recovery after death, ring of eight

| ring painted from | established | W at horizon | P at horizon |
| --- | --- | --- | --- |
| B0's measured bright stand | **1,500 (75 s)** | 0.0484 (12.3 % of B0's W) | 0.0609 (12.7 %) |
| the §11 hand table's bright stand | 1,500 (75 s) | 0.0495 (8.3 % of 0.60) | 0.0618 (11.0 %) |

**Criterion (§13.2, corrected in this round):** "establishes within a **few minutes**: each ring
donor splits its budget among **all** its bare neighbours in the stripped world, not only the
centre, so the §11 'eight donors' figure is an upper bound on the rate; rebuilding is reported
censored at 30 min."
**Verdict: met.** 75 s is within a few minutes, and the rebuild is censored as expected. Run 2
called 132 s "close to within a minute", which Astra correctly refused; with propagules off, the
B0-measured ring now holds enough reserve to be a donor from the first tick and both arms
establish at the same 1,500.

### B5 — dietary exclusion

| kind | cap_h | cap_d | (a) foliage | (b) charged litter | (c) placed carcass |
| --- | --- | --- | --- | --- | --- |
| burrower | 0.00 | 0.90 | died 9,788, ate **0.000** | died 13,900, ate 5.421 | died 7,774, ate 1.999 |
| grazer | 0.85 | 0.00 | died 6,435, ate 0.999 | died 7,420, ate **0.000** | died 7,420, ate **0.000** |
| glider | 0.90 | 0.00 | died 6,655, ate 0.999 | died 7,420, ate **0.000** | died 7,420, ate **0.000** |
| skimmer | 0.60 | 0.40 | died 7,224, ate 0.999 | died 8,333, ate 5.845 | died 8,476, ate 1.999 |

**Criterion (§13.2):** "grazer and glider starve on (b), (c); burrower starves on (a); **skimmer
lives on all three** at lower intake."
**Verdict: three clauses met, the skimmer clause unresolved.** The exclusions are exact — the
`7,420` rows are the pure-starvation baseline for a body given no food at all, and every masked
cell matches it to the tick. The skimmer clause is not met as written: it **draws on** all three
foods, which is the dependence the row is testing, but it dies on every one of them, at 7,224 /
8,333 / 8,476 ticks. Run 2 reframed "lives" as "draws on"; that reframing is withdrawn here. The
fixture gives each body a finite, non-renewing food and freezes every conversion, so nothing in
it can live indefinitely — which means the criterion as written is not testable in this fixture
either.

### B6a — reproduction on a finite input (3 × 3 bright, renewal off)

| measure | value |
| --- | --- |
| births / deaths | 2 / 4 starvation |
| **escrows opened** | **2** — so births ≤ escrows opened, and no birth was unfunded |
| material debited into escrow | 1.2000 m, 0.6000 m per escrow |
| peak population / extinct | 4 / 11,929 (596 s) |
| **complete bill over the run** | **9.1514 e owed**, 9.1509 e paid — mandatory 8.4559 e (4.70e-3 e/s), motor 0.6955 e (3.86e-4 e/s), both halves |
| unpaid (what the dying could not raise) | 0.0005 e |
| mean need in §11 foliage units | 4.383e-3 m/s |
| plant income | **0.0000 m** |
| cumulative `light_in − heat_out` | **−17.12 e** |

**Criterion (§13.2):** "births while the stock lasts, then starvation to zero; **no birth
without an escrow debit**; `U` non-increasing."
**Verdict: met — and now measured rather than asserted.** Run 2 claimed the escrow and
monotonicity clauses without demonstrating either. The escrow clause is now counted directly:
two escrows opened, two births, 0.6 m debited per escrow. `U` non-increasing per tick is the
claim A2b proves at machine precision on every tick of a world with everything on, including
care; the −17.12 e here is the cumulative consequence, and it is reported as the cumulative
figure it is.

### B6b — reproduction on a renewing patch (7 × 7 bright)

| window (ticks) | pop | births | deaths | Σ P | income m/s | eaten m/s | upkeep e/s | need m/s | **ratio** |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 4,000 | 4 | 2 | 0 | 21.660 | 6.08e-2 | 3.87e-2 | 2.01e-2 | 1.73e-2 | **3.52** |
| 8,000 | 4 | 3 | 1 | 18.508 | 5.30e-2 | 3.46e-2 | 2.41e-2 | 2.08e-2 | **2.55** |
| 12,000 | 6 | 5 | 1 | 13.319 | 4.18e-2 | 5.27e-2 | 2.90e-2 | 2.50e-2 | **1.67** |
| 16,000 | 7 | 6 | 1 | 8.117 | 2.69e-2 | 5.77e-2 | 4.03e-2 | 3.47e-2 | **0.78** |
| 20,000 | 8 | 8 | 2 | 3.481 | 1.31e-2 | 3.78e-2 | 4.36e-2 | 3.76e-2 | **0.35** |
| 24,000 | 8 | 8 | 2 | 0.914 | 3.68e-3 | 1.14e-2 | 4.42e-2 | 3.81e-2 | **0.10** |
| 28,000 | 3 | 8 | 7 | 0.074 | 4.72e-4 | 1.67e-3 | 1.68e-2 | 1.45e-2 | **0.03** |

Whole run: 8 births, 10 deaths, peak 9, extinct at 30,983 (1,549 s). **8 escrows opened for 8
births**, 4.8 m debited. The complete bill is **44.8542 e owed**, 44.8527 e paid — mandatory
41.9411 e (2.33e-2 e/s) and motor 2.9131 e (1.62e-3 e/s), both halves; the 0.0015 e gap is the
bill the dying left unpaid. Mean income 2.49e-2 m/s against a mean need of 2.148e-2 m/s —
**ratio 1.16**. First doubling 2 → 4 at tick 3,001 (150 s); B3's bright recovery to 0.5·P* on
the same stand is 823 s.

Units: income and need are both m/s. Upkeep is energy, converted through §11's own chain — a
unit grazer wins ≈1.16 e per metre of foliage bitten — so `need = upkeep / 1.16`. **Upkeep is
the complete bill the world booked**, read from the charging pass itself:
`MotorBill::total_cost` — maintenance, sensing and **both** halves of the motor charge,
translation and rotational sweep — over every body billed, **including any removed later in the
same tick**. It is not a reconstruction and it is not a bound.

**Repair cycle 3 corrected these figures.** Run 3 as first written reconstructed upkeep from
outside the step: `MotorBill::upkeep` over the bodies still alive *after* it, plus a travel term
from the distance each was transported. That dropped the rotational motor charge and every bill
a body paid in the tick it died, so the numbers were a lower bound on upkeep and the ratio an
upper bound on the surplus — which is not the "actual total upkeep" §13.2 B6b asks for (Astra's
cycle 2 verification). The measured motor half is 2.9131 e where the travel-only term gave
2.3161 e, so the rotational sweep is ~26 % of the motor bill; the whole-run ratio moves from
1.18 to **1.16**, and the window at 16,000 from 0.79 to **0.78**.

**Criterion (§13.2, rewritten in this round):** "per 1,000 ticks the region's plant income, the
**actual** total upkeep paid by every body alive, and their ratio; the first-doubling time
against the B3 bright time to `0.5·P*`; escrow debit per birth."
**Verdict: measured, and the surplus does not exist.** Run 2 reported a "2.24× surplus" and
called the extinction that followed a finding. Astra was right that the ratio was a
scenario-design artefact: it divided a per-stand hand yield by a **static** two-body need on a
population that peaked at 9. Measured against the bodies' own **complete** bill, the ratio is
**1.16 over the run, starts at 3.52 and falls below 1 between ticks 12,000 and 16,000** — the
population overtakes its food. The mechanism is in the doubling comparison: the population
doubles in **150 s** while the stand it eats needs **823 s** to recover half its foliage, 5.5×
slower. That is the coupled failure §13's B1/B6 pair was built to look for, and it is now
measured rather than inferred.

### B7 — establishment

| donor (pinned L·μ 0.60) | recipient | established | recipient at horizon |
| --- | --- | --- | --- |
| B0 measured | bright (0.60) | 11,089 (554 s) | W 0.0382 P 0.0473 Q 0.0191 |
| B0 measured | dim (0.20) | 11,089 (554 s) | W 0.0200 P 0.0122 Q 0.0100 |
| §11 hand | bright (0.60) | 6,139 (307 s) | W 0.0445 P 0.0551 Q 0.0223 |
| §11 hand | dim (0.20) | 6,139 (307 s) | W 0.0200 P 0.0109 Q 0.0100 |

**Criterion (§13.2, rewritten in this round):** "the donor's and the recipient's light and
moisture are pinned **per cell**, independently, so the donor is bright in both arms… the bright
cell establishes (~5 min with one donor) and shows positive foliage and income that grow; the
dim cell establishes and is **expected to die back**; both reported."
**Verdict: establishment met; the dim-recipient clause unresolved.** With the light now pinned
per cell, the confound Astra identified is gone and the result is clean: **both arms establish
on the same tick**, because establishment is paid entirely out of the donor's reserve and the
donor is bright in both. The recipient's own light then decides what it does afterwards — the
dim cell holds `W 0.0200, P 0.0122` against the bright cell's `0.0382 / 0.0473`, which is the
first clean measurement of the recipient's illumination in this milestone. The §11-donor bright
arm's 307 s matches §11's ~5 min. **No dim recipient died back** inside the horizon in either
arm, so that clause is censored, not contradicted.

## Run 2 — what the scenarios measured after the repaired §4.4

This is the current measurement. Run 1's tables are kept below, under their own heading, so the
change the repair made is visible rather than described.

### Run 2: B0 — stand baseline (the anchor)

| class | P | W | Q | F | Q_max | dP/dt | dW/dt | propagules sent | §11 hand P/W/Q |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| average | 0.0977 | 0.1050 | **0.0525** | 0.0000 | 0.0525 | 2.5e-5 | 0.0 | 0.0000 | 0.50 / 0.33 / 0.16 |
| bright | 0.4695 | 0.3578 | **0.0894** | 0.0047 | 0.1789 | 1.7e-4 | 1.5e-4 | **0.1660 m** | 0.56 / 0.60 / 0.30 |

**The repair works.** The average stand's reserve is now **exactly full** (`Q = q_cap · W`) and
the bright one is half full and still rising, against run 1's 0.0000 and 0.0435. The bright
stand crossed the §4.8 donor threshold and **sent 0.166 m of reserve to its bare neighbours** —
in run 1 nothing was ever sent by any simulator-grown stand. B0-1 is resolved.

**Finding R2-1, the cost of the share: B0 is now *further* from the §11 hand table, not
closer.** Bright `ΔP` went from −0.0422 to −0.0905 and `ΔW` from −0.1125 to −0.2422; average
`ΔP` went from −0.3232 to −0.4023 and its wood did not grow at all (`W` is still exactly the
seed `W_0 = 0.105`, `dW/dt = 0`). That is what `q_share = 0.2` costs at a 30-minute horizon: a
fifth of every surplus now goes to a stock the hand table assumed was already full, so foliage
and wood accumulate more slowly. §11 predicts this in words — "`P*` sits a few hundredths
lower until the reserve is full" — and the measured gap is larger than "a few hundredths"
because neither class finished filling inside the horizon. Both classes are still climbing;
this is a finite-horizon baseline, not equilibrium.

### Run 2: B1a — one pinned grazer, one bright mature stand, 12,000 ticks

| measure | run 2 | run 1 |
| --- | --- | --- |
| leaf / fruit bitten | 0.5473 / 0.0035 m | 0.5672 / 0.0504 m |
| feces | 0.2699 m | 0.3026 m |
| mean bite rate over its life | 2.31e-3 m/s (3.6×) | 2.43e-3 m/s (3.8×) |
| reserve-saturated ticks | 0 of 2,980 | 0 of 2,108 |
| stand at horizon | P 0.0006, W 0.3275, Q 0.0000, Wd 0.0294 | P 0.0006, W 0.4414, Q 0, Wd 0.0445 |
| stand death | censored | censored |
| grazer starved | 4,769 (238 s) | 5,082 (254 s) |

Unchanged in direction. The fruit intake fell tenfold because the B0 bright stand now carries
`F = 0.0047` instead of 0.0544 — foliage reaches the `fruit_min · P_max` ripening threshold
later when a fifth of the surplus is going to the reserve.

### Run 2: B1b — one mobile legacy grazer on a 5 × 5 mature region

| class | cells visited | Σ P painted → final | min stand P | stand deaths | grazer |
| --- | --- | --- | --- | --- | --- |
| bright | 66 | 23.037 → **0.0140** | 0.0004 | **18** | starved 21,877 (1,094 s) |
| average | 44 | 6.381 → **0.0460** | 0.0004 | 0 | starved 9,313 (466 s) |

**Finding R2-2: the repair did not fix B1b-1, and the bright arm got worse.** The grazer still
emptied the region — and now killed 18 of its 25 stands, where run 1 killed none. The reason is
visible in B0: a stand that spends a fifth of its surplus on reserve carries less wood
(`W` 0.358 against 0.487), so the same grazing pressure carries `W` below `W_min` inside the
horizon. The grazer itself lived 20 % longer (1,094 s against 913 s) because the reserve it was
indirectly eating had been stocked. Coexistence with retained foliage is still **not** observed
in the bright class.

### Run 2: B2 — depletion and relocation, three bright stands three cells apart

| stand | ticks on | min P | Q at departure | P at horizon | Q at horizon |
| --- | --- | --- | --- | --- | --- |
| (5,8) | 1,060 | 0.0003 | 0.0112 | 0.0003 | 0.0000 |
| (8,8) | 969 | 0.0003 | 0.0000 | 0.0003 | 0.0000 |
| (11,8) | 925 | 0.0003 | 0.0000 | 0.0003 | 0.0000 |

41 cells visited; grazer starved at 6,466 (323 s). **Changed direction from run 1.** In run 1
the grazer left each stand at `P ≈ 0.28–0.41` and all three recovered to `P ≈ 0.558`; in run 2
it stripped all three to `P = 0.0003` and **none recovered** by the horizon. The stands are
smaller (B0 bright W 0.358 against 0.487) so each holds less, and the grazer stayed ~1,000 ticks
on each instead of ~300. Recovery-before-return is still untested — the grazer was dead first.

### Run 2: B3 — plant recovery after defoliation

| arm | 0.5·P* | 0.9·P* | reserve dip → end |
| --- | --- | --- | --- |
| bright, B0-measured | **28,919 (1,446 s)** | censored | 0.0000 → **0.0894** |
| bright, §11 hand | 8,626 (431 s) | 25,755 (1,288 s) | 0.0000 → **0.1500** |
| average, B0-measured | censored | censored | 0.0000 → 0.0000 |
| average, §11 hand | censored | censored | 0.1600 → 0.0000 |

**B3-1 is resolved for bright.** In run 1 the B0-measured bright stand never reached even
0.5·P* and decayed for the whole run; now it recovers to half its foliage in 24 min **and
refills its reserve to 0.0894** on the way. The §11-hand arm also now refills (0.30 → 0 → 0.15)
where run 1 left it at 0.0486. Average is still censored in both arms — see the residual
findings.

### Run 2: B4a — three pinned grazers on one isolated bright mature stand

| measure | value |
| --- | --- |
| `Q` reached zero | 2,166 (108 s) — run 1: 914 (46 s) |
| dieback opened | 2,167 (108 s) |
| stand death | **censored, as §13.2 now expects** |
| `W` decline since dieback | 0.3580 → 0.2572 m over 1,692 s |
| mean `dW/dt` | 5.955e-5 m/s |
| measured e-folding time | **5,119 s (85 min)** |
| §11 prediction `1/(κ·m_w)` | 5,000 s (83 min) |
| dead wood at horizon | 0.0848 m |
| grazers starved | 8,098 (405 s) ×3 |

The corrected expectation is met precisely: the measured e-fold is within 2.4 % of `κ · m_w`.
The stand also held its reserve 2.4× longer than in run 1 before the grazers emptied it.

### Run 2: B4b — recovery after death, ring of eight

| ring painted from | established | W at horizon | P at horizon |
| --- | --- | --- | --- |
| B0's measured bright stand | **2,642 (132 s)** | 0.0466 (13.0 % of B0's W) | 0.0588 (12.5 %) |
| the §11 hand table's bright stand | 1,500 (75 s) | 0.0495 (8.3 % of 0.60) | 0.0618 (11.0 %) |

**Resolved.** In run 1 the B0-measured ring never sent anything at all; it now establishes in
132 s, close to §13.2's "within a minute" and to the §11-hand ring's 75 s. Rebuilding is
censored at 30 min in both arms, as expected.

### Run 2: B5 — dietary exclusion

Unchanged from run 1 to the digit — B5 freezes every conversion and paints its food directly,
so §4.4 never runs in it. The §6.1 exclusions hold exactly: burrower 0.000 on foliage; grazer
and glider 0.000 on litter and on carrion; skimmer eats all three.

### Run 2: B6a — reproduction on a finite input (3 × 3 bright, renewal off)

| measure | run 2 | run 1 |
| --- | --- | --- |
| births / deaths | 2 / 4 starvation | 2 / 4 |
| peak population | 4 | 4 |
| extinct | 15,232 (762 s) | 12,621 (631 s) |
| plant income over the run | **0.0000 m** | 0.0000 m |
| cumulative `light_in − heat_out` | **−15.42 e** | −16.97 e |

Expected direction met, unchanged: births while the stock lasts, then starvation to zero, with
plant income identically zero and `U` strictly falling.

### Run 2: B6b — reproduction on a renewing patch (7 × 7 bright)

| measure | run 2 | run 1 |
| --- | --- | --- |
| births / deaths | 6 / 8 | 8 / 10 |
| peak population | 8 | 10 |
| extinct | 28,571 (1,429 s) | 22,199 (1,110 s) |
| plant income | 34.319 m | 32.331 m |
| §11 surplus ratio | 2.24 | 2.24 |

**Finding R2-3: the repair did not fix B6b.** The population still goes extinct, 29 % later
than in run 1 and with two fewer births. A 2.24× production surplus still does not sustain two
grazers.

### Run 2: B7 — establishment

| donor | recipient | established | recipient at horizon |
| --- | --- | --- | --- |
| B0 measured | bright | **19,003 (950 s)** | W 0.0301 P 0.0370 Q 0.0151 |
| B0 measured | dim (L·μ 0.20) | censored | W 0.0071 P 0.0071 Q 0.0036 |
| §11 hand | bright | **6,139 (307 s)** | W 0.0445 P 0.0551 Q 0.0223 |
| §11 hand | dim (L·μ 0.20) | censored | W 0.0173 P 0.0173 Q 0.0086 |

**B7-1 is resolved.** The §11-donor bright arm now establishes in 307 s against §11's ~300 s
prediction, where run 1 took 1,146 s; the donor's reserve is no longer drained into its own
foliage, so the spend is continuous rather than intermittent. The B0-donor arm, which sent
nothing at all in run 1, now establishes in 950 s. **B7-2 is not resolved**: neither dim
recipient established or died back inside the horizon, in either arm.

## Run 1 — what the scenarios measured before the §4.4 repair

Kept unchanged, so the repair's effect is visible rather than asserted. Everything below this
heading is the first implementation run (`b1dd394`), against contract §4.4 as it stood before
commit `3316980`.

Every row is from one `--release` run of
`crates/cubarium-core/examples/ecology_v1_scenarios.rs`, censored at the 36,000-tick horizon
and never extended.

### Run 1: B0 — stand baseline (the anchor)

| class | P | W | Q | F | dP/dt | dW/dt | §11 hand P | §11 hand W |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| average | 0.1768 | 0.1091 | 0.0000 | 0.0000 | 4.2e-5 | 1.7e-5 | 0.50 | 0.33 |
| bright | 0.5178 | 0.4875 | 0.0435 | 0.0544 | 8.8e-5 | 2.1e-4 | 0.56 | 0.60 |

Both classes are **still climbing** at the horizon and neither has reached the §11 table. The
bright stand is within 8 % of the hand `P` and 19 % below the hand `W`; the average stand is
65 % below its hand `P`. Neither is equilibrium and this is not claimed to be.

**Finding B0-1, and the largest one in this note: the plant reserve `Q` never refills.** The
bright stand ends at 0.0435 — essentially the seed value 0.045 — against the hand table's 0.30,
and the average stand's reserve reaches exactly zero and stays there. §4.4 spends income on
foliage first, then wood, and only the remainder refills `Q`; with `D_P = r_p·W·dt` and
`D_W = r_w·W·dt` both proportional to `W`, a growing stand's income is fully absorbed before
the reserve is reached. Nothing is broken — the arithmetic is §4.4 exactly — but the
consequence is that a stand grown by the simulator never becomes what §4.8 calls a donor
(`Q > q_prop·Q_max`), and never holds the "finite budget for recovery" §4.4 describes. B3, B4b
and B7 below all turn on this.

### Run 1: B1a — one pinned grazer, one bright mature stand, 12,000 ticks

| measure | value |
| --- | --- |
| leaf bitten | 0.5672 m |
| fruit bitten | 0.0504 m |
| feces | 0.3026 m |
| mean bite rate over the 254 s it lived | 2.43e-3 m/s |
| ratio to §11's bright sustainable yield (6.4e-4) | 3.8× |
| reserve-saturated ticks | 0 of 2,108 |
| stand at horizon | P 0.0006, W 0.4414, Q 0.0000, Wd 0.0445 |
| stand death | censored at 36,000 |
| grazer starved | tick 5,082 (254 s) |

Expected direction met in kind: the stand is stripped to `P ≈ 0`, its reserve is spent to zero,
dieback opens, and the grazer starves. Two departures from §11's arithmetic:

- **B1a-1.** The realised bite is 3.8× the sustainable yield, not ~40×. §11's 40× is the bite
  at `P = 0.5` with a full mouth; the realised average includes the collapse, during which the
  type-II term `X/(X + K_P)` throttles the mouth hard (`K_P = 0.45` against a stand that is
  under 0.01 for most of the run). The direction is the same; the multiple is not.
- **B1a-2.** The reserve **never** saturates (0 of 2,108 requesting ticks), where §13.2 expected
  it to saturate first. `R_max` is 1.0 m and the grazer took 0.62 m in total.
- **B1a-3.** The stand does **not** die inside the horizon. Dieback is `κ · unpaid` with
  `unpaid = m_w·W·dt ≈ 3.5e-6` per tick, so carrying `W` from 0.44 down to `W_min = 0.02` needs
  on the order of 10⁵ ticks. "The stand dies" is true of the mechanism and false of the horizon.

### Run 1: B1b — one mobile legacy grazer on a 5 × 5 mature region

| class | region production (§11) | cells visited | Σ P painted → final | min stand P | stand deaths | grazer |
| --- | --- | --- | --- | --- | --- | --- |
| bright | 25 × 6.4e-4 = 1.6e-2 m/s | 66 | 27.578 → 0.0099 | 0.0003 | 0 | starved at 18,252 (913 s) |
| average | 25 × 1.5e-4 = 3.8e-3 m/s | 120 | 7.148 → 5.377 | 0.0748 | 0 | starved at 5,260 (263 s) |

**Finding B1b-1 — a bright failure, which §13.2 names as a finding.** Bright coexistence with
retained foliage was expected: the region's production is 1.6e-2 m/s against a cruising need of
6.9e-3. It did not happen. The single grazer ran the 25-cell region from 27.6 m of standing
foliage down to 0.01 m and then starved. The region's *production* exceeds the animal's *need*,
but production is not what the mouth meets: a stand only regrows at `r_p·W` and the bite is
`mouth_rate · X/(X + K_P)`, so the grazer strips faster than the patch renews and then cannot
recover its own upkeep from what is left. Whether this is the animal's foraging, the
plant-side rates or the type-II half-saturation is for the whole-ecosystem tuning, not this
slice.

The average arm behaved as expected: the grazer ran the region down (Σ P 7.15 → 5.38 with a
minimum of 0.075) and starved.

### Run 1: B2 — depletion and relocation, three bright stands three cells apart

| stand | ticks on | min P | Q at departure | P at horizon | Q at horizon |
| --- | --- | --- | --- | --- | --- |
| (5,8) | 461 | 0.2816 | 0.0369 | 0.5578 | 0.0267 |
| (8,8) | 283 | 0.3836 | 0.0426 | 0.5579 | 0.0736 |
| (11,8) | 226 | 0.4133 | 0.0434 | 0.5579 | 0.0857 |

The grazer visited 97 cells and starved at tick 4,469 (223 s). It left each stand at
`P ≈ 0.28–0.41`, above the type-II floor `K_P = 0.45` rather than at it — the mouth is throttled
long before the stand is empty. Every stand recovered fully (to `P ≈ 0.558`) by the horizon,
which is reported rather than assumed; the grazer was dead by then, so recovery-before-return
was never tested.

### Run 1: B3 — plant recovery after defoliation

| arm | reached 0.5·P* | reached 0.9·P* | reserve dip → end |
| --- | --- | --- | --- |
| bright, B0-measured stand | censored | censored | 0.0435 → 0.0000 |
| bright, §11 hand stand | 8,626 (431 s) | 19,129 (956 s) | 0.3000 → 0.0486 |
| average, B0-measured stand | censored | censored | 0.0000 → 0.0000 |
| average, §11 hand stand | censored | censored | 0.1600 → 0.0000 |

**Finding B3-1 — recovery is decided entirely by the reserve, and the B0-measured stand has
none.** A stripped stand's income is `c·P` with `P = 0`, so the only foliage it can make is
reflush from `Q`. The B0 bright stand's 0.0435 m of reserve buys `0.0435/(1+c_g) = 0.036` m of
leaf, below the §11 breakeven of `P ≈ 0.07`, so income never covers maintenance again and the
stand decays for the rest of the run. The §11 hand stand's 0.30 m of reserve buys 0.25 m, well
above breakeven, and recovers on almost exactly §11's predicted schedule (0.9·P* at ~16 min
against the predicted ~10 min). Average is censored in both arms, as §13.2 expected.

Because the contract is ambiguous about what "mature stand" means once B0 has run (§13 says
both "the §11 steady-state values" and "the measured values become the mature stand"), B3, B4b
and B7 are each run **twice**, once from each, and both are reported. See "contract
interpretations" below.

### Run 1: B4a — three pinned grazers on one isolated bright mature stand

| measure | value |
| --- | --- |
| `Q` reached zero | tick 914 (46 s) |
| dieback opened | tick 915 (46 s) |
| stand death | censored at 36,000 |
| dead wood at horizon | 0.1189 m (living W 0.3455, P 0.0002) |
| grazers starved | 8,200 / 8,200 / 8,200 (410 s) |

`Q → 0` and `W → Wd` as expected, with no regrowth (there is no donor). The stand's death is
censored for the same reason as B1a-3: dieback at `κ · m_w · W · dt` is far slower than the
horizon.

### Run 1: B4b — recovery after death, ring of eight

| ring painted from | established | W at horizon | P at horizon |
| --- | --- | --- | --- |
| B0's measured bright stand | censored at 36,000 | 0.0000 (0 % of B0's W) | 0.0000 |
| the §11 hand table's bright stand | tick 1,500 (75 s) | 0.0588 (9.8 % of 0.60) | 0.0696 (12.4 % of 0.56) |

With §11's stand the ring establishes in 75 s — §13.2 expected "within a minute" — and the
rebuild is censored at 30 min, also as expected. With B0's measured stand **nothing is ever
sent**: no ring cell clears `q_prop · Q_max`, so §4.8 never fires (finding B0-1).

### Run 1: B5 — dietary exclusion

Conversions frozen (`m_p = ripen = drop = k_d = k_c = k_w = fall = 0`), bodies pinned, each
animal built from its full default founder kind (diet, size, metabolism, speed, depth, swim).

| kind | cap_h | cap_d | (a) foliage | (b) charged litter | (c) placed carcass |
| --- | --- | --- | --- | --- | --- |
| burrower | 0.00 | 0.90 | died 9,788, ate **0.000** | died 13,900, ate 5.421 | died 7,774, ate 1.999 |
| grazer | 0.85 | 0.00 | died 6,435, ate 0.999 | died 7,420, ate **0.000** | died 7,420, ate **0.000** |
| glider | 0.90 | 0.00 | died 6,655, ate 0.999 | died 7,420, ate **0.000** | died 7,420, ate **0.000** |
| skimmer | 0.60 | 0.40 | died 7,224, ate 0.999 | died 8,333, ate 5.845 | died 8,476, ate 1.999 |

The exclusions are exactly §6.1's: the burrower takes nothing from foliage, the grazer and
glider take nothing from litter or remains, and the skimmer eats all three. The `7,420` rows
are the pure-starvation baseline for a grazer-sized body with no food at all, so a kind that
"died 7,420" starved as if the cell were empty — which, to its machinery, it was.

**B5-1.** Every body eventually died, including the skimmer, because each fixture holds a
finite, non-renewing food (1 m of foliage, 2 m of litter, 2 m of carcass) and B5 freezes every
conversion. §13.2's "the skimmer lives on all three" is met in the sense that matters —
it is the only kind that draws on all three — but it is not immortal on them, and the test
measures dependence rather than viability, as §13.2 says.

### Run 1: B6a — reproduction on a finite input (3 × 3 bright, renewal off)

| measure | value |
| --- | --- |
| births | 2 |
| deaths | 4 starvation, 0 age, 0 collapse |
| peak population | 4 |
| extinct | tick 12,621 (631 s) |
| intake | leaf 4.952 m, fruit 0.319 m |
| plant income over the run | **0.0000 m** |
| cumulative `light_in − heat_out` | **−16.97 e** |

Exactly the expected direction: births while the stock lasts, then starvation to zero, with
plant income identically zero and `U` strictly falling. Every birth was paid — A2b and A1 cover
the escrow accounting at machine precision, and the world's own debug audit ran on every tick
of this run.

### Run 1: B6b — reproduction on a renewing patch (7 × 7 bright)

| measure | value |
| --- | --- |
| births | 8 |
| deaths | 10 starvation |
| peak population | 10 |
| extinct | tick 22,199 (1,110 s) |
| intake | leaf 35.367 m, fruit 2.556 m |
| plant income over the run | 32.331 m |
| cumulative `light_in − heat_out` | −65.50 e |
| §11 ratio | 49 × 6.4e-4 = 3.14e-2 m/s against 1.4e-2 for two cruising grazers = **2.24** |

Births happened, as expected. They were followed by starvation to extinction, which §13.2 says
is measured rather than assumed — so this is the measurement, and it is the same shape as
B1b-1: a production surplus of 2.24× did not keep a population alive, because the mouths meet
stocks rather than production.

### Run 1: B7 — establishment

| donor | recipient | established | recipient at horizon |
| --- | --- | --- | --- |
| B0 measured | bright (L·μ 0.60) | censored | W 0.0000 |
| B0 measured | dim (L·μ 0.20) | censored | W 0.0000 |
| §11 hand table | bright (L·μ 0.60) | 22,912 (1,146 s) | W 0.0290 P 0.0337 Q 0.0100 |
| §11 hand table | dim (L·μ 0.20) | censored | W 0.0056 P 0.0056 Q 0.0028 |

**B7-1.** With a §11 donor the bright cell establishes at 1,146 s, not the ~5 min §11 predicted.
§11's estimate assumes the donor spends `k_est · dt` every tick; the measured donor spends only
what its reserve holds above `q_prop · Q_max`, which recovers slowly, so the transfer is
intermittent. With a B0 donor nothing is sent at all (finding B0-1).

**B7-2.** Neither dim recipient died back inside the horizon, so §13.2's "the dim cell
establishes and is expected to die back" is **not observed**: the dim cell did not finish
establishing either. Both are reported as censored rather than resolved.

## Contract clauses that had to be interpreted

All eleven below were **accepted** in contract §18's third round; interpretation 6
(`producer.energy_density` removed in favour of the one `e_v`) is now recorded in §14. They
are kept here as the record of what was decided and why.

Repair cycle 1 added no new interpretation: the revised §4.4 is unambiguous, and
`reserve_share` / `reflush_below` are the two §14 fields with their stated `[0, 1]` validation.
One arithmetic coincidence in §11 is worth naming, and it is a measurement rather than a
reading — see finding R2-4.

**Repair cycle 2 added one implementation choice**, and it is the only new public API in the
milestone. §13.2's B7 row now requires the donor's and the recipient's light and moisture to be
pinned **per cell**, and nothing in the world could express that: the habitat is a pure function
of `config.habitat` and the seed, which is uniform or smooth by construction, and
`World::from_state` rebuilds it. `World::pin_cell_habitat(cell, light, moisture)` writes one
cell's base light and moisture and re-samples, so a staged fixture can light two adjacent cells
differently. It is transient — not persisted, not hashed, dropped by a `from_state` round trip —
and it changes no equation, no ordering, no parameter and no draw.

Each of these was ambiguous or under-determined in the contract; none of them is a change to a
§4–§10 equation, a §4.0 read/write, a §11 value or a §15 rule.

1. **"Mature stand" after B0 (§13 vs §13.2).** §13 defines a mature stand as "the §11
   steady-state values of that class painted at tick 0" and, two sentences later, says B0's
   "measured values become the 'mature stand' every later scenario is judged against". Under
   §11's provisional numbers those are different stands in the one stock §4.8 reads. **Taken
   both ways**: B3, B4b and B7 run one arm from each, clearly labelled, and both are reported.
   B1a, B1b, B2, B4a, B6a and B6b paint B0's measured values, as the handoff directs.
2. **Whether §4.8 runs during B0.** §13.2's B0 is "one lone stand … no animals", and §13 says
   the world is "stripped to the named cells" — but a bare cell is still a legal propagule
   recipient. Propagules were left **on**: disabling a §4 mechanism for a baseline would make
   B0 measure something other than the model. In the event the question was moot — a B0 stand
   never clears the donor reserve floor, so nothing was ever sent.
3. **The cell class boundary at `W_min = 0`.** §3.1 defines alive as `W⁻ ≥ W_min`, bare as
   `W⁻ = 0`, and establishing as `0 < W⁻ < W_min`; at `W_min = 0` the first and third overlap.
   Read as: **bare** is `W⁻ = 0`, **alive** is `W⁻ > 0 and W⁻ ≥ W_min`, **establishing** is the
   rest. At the §11 value `W_min = 0.02` this is the contract's own partition exactly.
4. **3c on a frozen cell.** §4.0 marks ripening and drop "all" cells, while §3.1 freezes an
   establishing cell against "income, maintenance, growth, senescence, dieback, death" —
   ripening is not in that list. Implemented as the table says: 3c runs on every cell.
   Unreachable in practice, since a propagule's foliage is orders of magnitude below
   `fruit_min · P_max`.
5. **Two sequential `e_d_max` clamps in 3b and 3c.** §4.5 clamps litter energy at `e_d_max·D²`
   and §4.3's drop clamps again at `e_d_max·D³`. Because `D³ ≥ D²`, clamping twice dumps
   slightly more heat than one combined clamp would. Implemented **as written** (sequential),
   which is the conservative reading.
6. **`e_v` and `e_p`.** §11 makes `e_v` "one density for all plant tissue", equal to today's
   `e_p`. Two config knobs for one quantity is a defect in an accounting identity, so
   `ProducerConfig.energy_density` was **removed** and `PlantConfig.energy_density` is the one
   `e_v` for `P`, `W`, `Q` and `Wd`. `params.rs`'s exclusion list names `plant.energy_density`
   now; §15.3's promise that "`params.rs` names stay valid" holds for every *searched*
   parameter, which this never was.
7. **`Q_0`.** §11 gives the initial reserve as `0.5 · Q_max0`, and §14 fixes `PlantConfig` to
   fifteen named fields with no room for that fraction. It is a documented module constant,
   `fields::INITIAL_RESERVE_FRACTION`, not a new knob.
8. **Where `EcologyV1State` is defined.** §14 puts it in `world/state.rs`. The **field** on
   `WorldState` is there; the **type** is in `fields.rs` beside the subphases that operate on
   it, and re-exported as `cubarium_core::world::EcologyV1State`. Same wire shape, same
   trailing position.
9. **"Feeding threshold per stock" (§14, controller).** Read as the existing `feed_min` gate
   applied per channel, with the detrital channel reading `D_eff + C_eff` — which is what §6.2
   and §9 describe. No new threshold was added.
10. **A hunter's carried carcass on its death.** §8 says "a hunter member's death goes to `C`"
    and §5 lists "a hunter member's death" among `C`'s sources, without separating the body
    from the gut. Both go to `C`: a carcass a hunter was carrying is a body, not plant litter.
    Its digestion *rejects* go to `D`, as §8 says explicitly.
11. **`ecology_hash`.** §15.1 says to keep it "only if the care/no-care comparison still uses
    it". It does (`tests/care_replay.rs`, `examples/care_compare.rs`, `quiet_compare`,
    `hunter_compare`, `Telemetry`), so it is kept — with the consequence, named here, that the
    schema 7 projection cannot cover the ecology v1 pools, so two worlds differing only in wood
    now hash alike under `ecology_hash`. `state_hash` covers everything and is what every
    replay check in the suite uses.

## §11 values `validate` forced to change

**None, in either run.** Every §11 provisional value — including repair cycle 1's `q_share` 0.2
and `p_reflush` 0.25 — is the shipped default and every one is admitted by
`WorldConfig::validate`. The validations (`rate · dt ≤ 1` by name for `m_w`, `m_p`, `ripen`,
`drop`, `k_d`, `k_c`, `k_w`, `fall`; `alive_min < donor_min ≤ wood_max`; `propagule_split`
summing to 1; `capability_gate ∈ [0, 0.5]`; `capability_exponent > 0`; and now
`reserve_share ∈ [0, 1]`, `reflush_below ∈ [0, 1]`) all pass at the §11 table with room to
spare.

## Old-schema refusal tests, and the retired continuations

Schema 16 refuses every schema from 7 to 15 by name (`SnapshotError::UnsupportedSchema`). The
`From<WorldStateVn> for WorldState` conversions and `v10::migrate` are **deleted**; the frozen
mirror shapes, their `SCHEMA_Vn` constants and the `project` functions remain, because the
refusal tests and `ecology_hash` still use them.

Refusal tests, each asserting the named version on the real recorded bytes (and, where the
provenance recorded one, each file's own payload hash first):

| test | fixtures |
| --- | --- |
| `ecology_v1::a7_schema_sixteen_round_trips_and_every_older_schema_is_refused_by_name` | the rule itself, schemas 7–15 |
| `snapshot::tests::every_older_schema_is_refused_by_name` | the rule, in-crate |
| `snapshot::tests::a_relabelled_schema_sixteen_payload_is_refused` | trailing-byte relabel |
| `snapshot_hardening::the_schema_version_is_sixteen_and_every_predecessor_is_refused` | every version 0–16 |
| `continuation_fixtures::every_retired_fixture_is_present_and_refused_by_name` | all 25 `.cubw` files |
| `care::the_live_schema_seven_fixtures_are_refused_by_name` | `live-v7-55200*` |
| `energy_correction::the_pre_correction_fixtures_are_refused_by_name` | `live-v7-55200`, `live-v8-172800*` |
| `hunter_migration::the_pre_hunter_schema_nine_fixtures_are_refused_by_name` | `pre-hunter-v9-173400*` |
| `hunter_migration::every_schema_ten_payload_is_refused_by_name_whatever_it_carries` | hand-framed schema 10 |
| `care_dose_migration::the_pre_dose_schema_eleven_fixtures_are_refused_by_name` | `care-v11-*` |
| `quiet_migration::the_pre_quiet_schema_twelve_fixtures_are_refused_by_name` | `quiet-v12-*` |
| `astra_quiet_policy::the_schema12_quiet_fixtures_are_refused_by_name` | `quiet-v12-*` |
| `hunter_charging::the_pre_change_charge_fixtures_are_refused_by_name` | `hunter-v3-charge-*` |
| `astra_target_cleanup::the_retained_seed6_artifacts_are_refused_by_name` | the seed-6 capture |
| `apex_dormancy::dormant_state_round_trips_and_schema_thirteen_is_refused` | hand-framed schema 13 |
| `neural_runtime::a_hand_framed_schema_fourteen_payload_is_refused_by_name` | hand-framed schema 14 |

Retired continuation comparisons — the load-and-step-600 checks and the `*-plus600-r0b` /
`-r0d` recordings that anchored them. **No fixture file was deleted or regenerated**; each
provenance note records the retirement.

| retired comparison | fixtures | provenance note |
| --- | --- | --- |
| schema 7 care migration + continuation | `live-v7-55200*` | `live-v7-v8-provenance.md` (new) |
| schema 8 correction migration + continuation | `live-v8-172800*` | `live-v7-v8-provenance.md` (new) |
| schema 9 empty-hunter continuation | `pre-hunter-v9-173400*` | `pre-hunter-v9-provenance.md` |
| schema 11 pre-dose shower and hunter continuations | `care-v11-*` | `care-v11-provenance.md` |
| schema 12 pre-quiet plain and care continuations | `quiet-v12-*` | `quiet-v12-provenance.md` |
| schema 12 profile-3 charging continuation | `hunter-v3-charge-*` | `hunter-v3-charge-provenance.md` |
| the schema 12 seed-6 exact artifact replay | `captures/…/seed-6/` | retirement recorded in the test |
| the whole regenerator (`continuation_fixtures::regenerate`) | all of the above | the file's own header |

Three claims those tests carried were **rebuilt on worlds this build makes** rather than
retired, because they were never about a particular recording: the schema 7/8/9 projections are
still checked as strict prefixes of the schema 16 payload
(`hunter_migration::the_older_projections_still_drop_only_what_they_are_named_for`); the
charging policy's candidate band is still separated by a hand-built member
(`hunter_charging::a_member_inside_the_band_separates_the_two_policies`); and the quiet pause
suite now warms its own mature world for 6,000 ticks instead of loading the schema 12 fixture.

## The runtime edits, and nothing else

Exactly the four §15.2 edits were made in `neural/`:

1. `PROFILE_TEXT` gains `|eco:v1`, so the schema digest changes and every existing policy file,
   every `runs/es-*` centre and the R3a display seed is refused by name. No `runs/es-*` campaign
   is resumable and the R2b/R2c/R2d results are not evidence about this ecology.
2. `Capability` carries `cap_foliage` and `cap_detrital` instead of `diet`; the graze and fruit
   masks open on `cap_foliage > 0`, scavenge on `cap_detrital > 0`.
3. `Feedback::channels` normalises all three `ate` channels by the one `mouth_rate`.
4. The observation sampler's `d_here` and `food_*` detrital channels read `D_eff + C_eff`.

The observation layout, the action layout, `Gru32`, the optimizer, the trainer loop, the export
format and the motor contract are untouched. `GRU_PARAMETERS` is still 10,215 and
`tests/neural_runtime.rs` still passes unchanged apart from the retired schema 14 migration.

## Other consequences worth naming

- **A fresh world starts with far less foliage.** `P_0` is now `initial_fraction · min(P_max,
  α · W_0)` rather than `initial_fraction · P_max · L₀ · μ₀`; the brightest cell opens near
  `P = 0.24` instead of `0.60`, which is below the `fruit_min · P_max = 0.45` ripening
  threshold. A default world therefore holds **no fruit at all** after 100 s, where the
  pre-ecology-v1 world did. Two tests that asserted "a default world ripens some fruit in
  100 s" now assert zero and exercise ripening on a world rich enough to do it.
- **`mass_residual` tolerances moved from 1e-12 to 1e-9.** The identity now sums eight 1,280-term
  vectors instead of four, and the extra summation rounding is ~1e-12 absolute on a world
  holding ~660 m. 1e-9 is the contract's own A1 bound (§13.1).
- **Telemetry consumers** that summed `detritus` as "all dead matter" now need
  `detritus + carrion + dead_wood` (§15.3). `Telemetry` gains `wood`, `plant_reserve`,
  `dead_wood`, `carrion`, `carrion_energy`, `bare_cells`, `establishing_cells`, `plant_deaths`
  and `recolonisations`.
- **`RenderView` and `FieldDump` carry every new pool** and nothing draws them. The presentation
  task of §12 needs no further core change.
- **`Layout::build` in `cubarium-search`** paints `W = P/α` and `Q = q_cap·W` with its foliage,
  and `painted_material` counts all three. The layout and protocol hashes change by
  construction; the optimizer, trainer loop and export format are untouched.
- **`hunter.rs::observations_do_not_move_a_profile_three_hunter_world` was re-recorded twice**,
  once for ecology v1 and once for repair cycle 1. Its schema 12 projection covers `config` and
  `fields`, and both moved each time — the config gained the `plant` block and then its two new
  fields, and the plant arithmetic changed. The claim the test makes, that *observing* a world
  never moves it, is unchanged; only its build-specific anchor is.
- **Repair cycle 1 touched exactly three source files**: `config.rs` (the two fields and their
  validation), `fields.rs` (the revised §4.4 block), and the B4a reporting in the scenario
  binary. Nothing in `neural/`, `snapshot.rs`, `step.rs` or `cubarium-search` moved.
- **Repair cycle 2 touched four**: `fields.rs` (one line — 3h's class now comes from the
  post-3d wood), `snapshot.rs` (`ecology_hash`), `world/view.rs` (`pin_cell_habitat`), and
  `cubarium-search/src/params.rs` (the exclusion text). Nothing in `neural/`, `step.rs`,
  `config.rs` or the optimizer moved, and no parameter value changed in any cycle.
- **Repair cycle 3 touched two**: `world/state.rs` (three transient `body_bill_*` counters on
  `IntakeDiagnostics`) and `world/step.rs` (three accumulation lines at the two sites that
  already levy the charge — the movement pass and the dormancy pass). **No bill changed**: the
  counters read `MotorBill::total_cost`, `MotorBill::upkeep` and the amount actually collected,
  all of which the step already computed. Nothing is persisted or hashed, and the figures are
  zero again after a reload, like every other field on that struct.
- **`ecology_hash` now covers the ecology**, which retires run 1's open finding 7. A consumer
  that compared a care run with a no-care run still compares — only care is masked — but a
  reader should know the number changed meaning: it is the current state, not a schema 7
  projection, so it will not match any hash recorded before this milestone.

## Findings after repair cycle 2

### What Astra's review asked for, and what changed

| finding | change | evidence it took effect |
| --- | --- | --- |
| **1 (P1) 3h read the pre-tick class, not post-3d `W⁴`** | `work.class` is now written from the post-3d wood inside the per-cell block, so a stand that dies in 3d is bare to 3h on the **same** tick. Donor eligibility is unchanged by this — a dead cell has `W⁴ = 0 < W_est` under either reading. | `a9b_a_stand_that_dies_in_3d_receives_a_propagule_in_the_same_tick`: the stand dies on tick 1 and receives 8.33e-6 m of propagule on tick 1. Under the old reading it would have received nothing. |
| **2 (P1) `ecology_hash` blind to every ecology v1 pool** | It is now the FNV-1a of the postcard encoding of the **current** `WorldState` with `care = CareState::default()` and nothing else altered (§15.1). | `a7b_the_ecology_hash_moves_with_every_ecology_stock_and_not_with_care`: each of the five ecology vectors perturbed by 1e-9 in three cells apiece, both counters, and six pre-existing fields all move the hash; a fully populated care ledger does not; `state_hash` and `ecology_hash` agree exactly when care is empty and differ when it is not. Four stale claims elsewhere in the suite were corrected to the new definition, and `care_replay.rs` passes unchanged. |
| **3 (P2) B0, B1b, B6b and B7 did not measure their claims** | B0 holds `propagule_rate = 0` with a B0x arm reporting the export; B1b reports foliage `ΣP` apart from `Σ(P+W+Q+F)`; B6 measures the income/need ratio in one unit, the first-doubling time and the escrow debit behind every birth — and, since repair cycle 3, the **complete** bill from the charging pass itself; B7 pins donor and recipient light per cell through the new `World::pin_cell_habitat`. | B0 bright reserve 0.0894 → **0.1967** (full) once the export stops. B1b bright foliage is **11.972 m**, not run 2's mislabelled 23.037. B6b's measured ratio is **1.16**, not 2.24, and falls below 1 between ticks 12,000 and 16,000. B7's two arms now establish on the **same** tick, isolating the recipient's light. |
| **4 (P2) A1/A3b/A4/A6/A9 could pass a wrong implementation** | Five strengthenings, listed below. | `cargo test -p cubarium-core` 466 passed (461 before). |
| **5 (P2) the note called censored results passes** | The whole run 3 section is written criterion-first, and every verdict is *met* or *unresolved* against the literal §13.2 text. | B1a, B2, B5's skimmer clause and B7's dieback clause are now **unresolved**; B3 and B4b are met only because §13.2's expectations were themselves corrected in this round. |
| **6 (P2) `params.rs` advertised `producer.energy_density`** | The exclusion entry names `plant.energy_density` and explains the fold; a second entry excludes the whole `plant.*` block per §15.3. | Two new tests: every searched name applies to a schema 16 config, round-trips bit-exactly through `apply`/`read`, and validates; and the exclusion list names no removed key. |
| **7, 8 (P3)** | Nothing to change. The three tuning questions (B1b-1, B3 average, R2-4) are carried forward unchanged. | — |
| **cycle 2 verification: B6's upkeep was a lower bound** | Repair cycle 3. `IntakeDiagnostics` gains `body_bill_total`, `body_bill_paid` and `body_bill_upkeep`, accumulated where the charge is levied, so the complete bill — both halves of the motor charge included — is counted for every body billed, whether or not it survives the tick. B6a and B6b read it and were re-run. | `the_exposed_body_bill_is_the_sum_of_every_bill_including_a_body_that_dies_this_tick`: four bodies, one with no stores, on a fixture with turn noise on. Exposed mandatory 1.240e-3 e equals `Σ MotorBill::upkeep` over **all four** and exceeds the survivors' 9.300e-4 e; the motor half is 4.813e-5 e and positive; the owed-minus-paid gap is exactly the dying body's whole bill. B6b's whole-run ratio moves 1.18 → **1.16**. |
| **9 (P2) cargo was blocked in Astra's sandbox** | The four commands were run here. | The summaries are in "Verification" above: 466 / 68 / 569 passed, 0 failed, scenarios exit 0 in 71 s. |

The five test strengthenings, each answering a specific way the old test could pass a wrong
implementation:

- **A1** gains `a1_every_stock_moves_only_through_the_transfer_that_names_it`: six arms that
  switch off every rate but one and assert the **exact** movement of all ten stock totals — the
  one that should move and the nine that must not. `mass_residual` is one number and cannot see
  a paired omission; these can.
- **A3b** gains the **interior** joint-withdrawal case, `k·dt = 0.3` with `fall·dt = 0.5`, for
  both litter and remains. At the coincident endpoint the correct rule makes fall zero, so an
  implementation that simply suppressed fall whenever decomposition ran would pass; here the
  source must keep exactly `X⁻(1 − k·dt)(1 − fall·dt)`, the whole stock must lose exactly the
  decomposition, the density must not move, and the heat must be exactly `ρ · dec`.
- **A4** gains `a4_every_one_of_the_four_foods_books_material_and_energy_exactly`: one isolated
  bite of leaf, fruit, litter and remains, with every §6.4 term asserted — stock material and
  energy, reserve, battery, feces, and heat as an **equality** rather than a bound, because the
  fixture pays no other bill. The detrital foods are given `ρ = 0.75 · e_r`, so the
  `min(1, ρ/e_r)` factor is genuinely engaged.
- **A6** now asserts the hunter's death **exactly**: remains gain the body plus the gut, the
  energy is each deposit's own `e_c_max` clamp, the excess is heat, and the post-death residual
  (`gut_material_total`, `gut_energy_total`) is zero. Ordinary death and miscarriage gained
  their exact energy terms and clamps too.
- **A9** gains two cases: `a9b` (a stand dies in 3d and receives a propagule the same tick) and
  `a9c` (two donors with **different** budgets sharing three recipients each — a shared
  recipient receives the sum, a private one receives its own donor's share, and the richer
  donor's recipients receive more, which a pooled or last-writer-wins commit would fail).
- **A2b**'s tautological `plant_deaths_total > 0 || tick > 0` is replaced by seven claims the
  fixture must actually satisfy: all four foods eaten, plant income earned, a propagule sent, a
  birth and a death — and the fixture was restocked so it meets them.

### Still open, carried to the later whole-ecosystem search

No parameter was changed in any cycle, and none is proposed here.

1. **B1b-1 — bright coexistence fails.** 61 cells visited, foliage 11.972 → 0.020 m, 17 of 25
   stands dead, grazer starved at 1,262 s. Astra classified it a **later tuning question**
   (finding 8, item 1) once the reporting was corrected, and the corrected reporting does not
   change the outcome.
2. **B6b — the population overtakes its food.** Measured ratio **1.16** over the run against
   the bodies' complete bill, starting at 3.52 and below 1 between ticks 12,000 and 16,000;
   doubling in 150 s against an 823 s recovery. Now a measurement rather than a scenario
   artefact, and a tuning question.
3. **B3 average censored in both arms** — §13.2's own expected direction, and a tuning question
   (finding 8, item 5).
4. **R2-4 — `p_reflush · α = q_cap` at §11's values**, so a full reserve buys only `Q_max/(1+c_g)`
   of leaf and the reserve binds before the reflush ceiling. Astra confirmed the equality and
   that no implementation deviation follows (finding 8, item 8). Tuning question.
5. **B1a's ordering and stand death, B2's recovery-before-return, B5's skimmer survival, B7's
   dim dieback** are all **censored or untestable in their fixtures** at the 30-minute horizon.
   They are reported as unresolved above rather than as passes; each would need either a longer
   horizon, which §13 forbids, or a different fixture, which is not this milestone's to design.
6. **R2-1 is reduced, not withdrawn.** With the propagule export removed, bright `W` is 0.3934
   against run 1's 0.4875, so `q_share` does cost growth at a 30-minute horizon — but a third of
   run 2's reported gap was the export, and this note said otherwise.

## Stop

Held-out checks, training in the revised ecology, display deployment and the §12 presentation
task are separate assignments after the review.
