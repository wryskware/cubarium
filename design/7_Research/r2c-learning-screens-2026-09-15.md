---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# R2c: three one-variable learning screens and a first held-out check — 2026-09-15

Executed by Fable after [R2b](r2b-first-learning-result-2026-09-15.md) under Wrysk's
"do whatever makes sense" with the 20-worker allowance. R2b's diagnosis was that the
survival objective is nearly a step function where the centre sits, so the screen
asked three questions, one variable each against the R2b baseline (16 pairs, 16
updates, minimum over layouts, seed 20260915), with the optimizer and fixtures frozen:

1. **Mean instead of minimum** over the four layouts, same budget.
2. **Minimum, 64 updates** instead of 16.
3. **Minimum, 32 pairs** instead of 16, 16 updates.

**Headline.** Two of the three produced a policy that forages. The 64-update
minimum-score run reached the full 1,800 s horizon, funded, on all four training
layouts from generation 58 on, and its generation-59 centre then survived **seven of
eight unseen held-out layouts** funded. The mean-score run got there in 6 updates on
three layouts but never solved the weak-opening layout, and it transfers worse
(five of eight). Doubling the population did nothing in 16 updates. This is the first
trained recurrent policy in the project that actually eats.

## 1. What ran

| Screen | Command delta from R2b | Protocol hash | Updates | Episodes | Ticks | Wall |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| R2b baseline | (none) | `0xb69033f65e56f1df` | 16 | 2,116 | 16.3 M | 68.7 s |
| mean16 | `--aggregate mean` | `0xd6f90c275924f0a1` | 16 | 2,116 | 51.3 M | 208.5 s |
| min64 | `--generations 64` | `0xb69033f65e56f1df` | 64 | 8,452 | 114.9 M | 478.7 s |
| pairs32 | `--pairs 32` | `0x504cc5546a5d0c6d` | 16 | 4,164 | 32.0 M | 129.0 s |

All at `--workers 20 --wall-seconds 1200 --horizon 36000 --train-seed 20260915
--center-eval true`, build `2f7a59e215a2-dirty` plus the aggregate commit `47af298`
(the working tree carried unrelated uncommitted files outside `crates/`, hence
`-dirty`; the search crate itself was committed). Zero discarded work in every run.
Outputs under `runs/es-r2c-{mean16,min64,pairs32}/` (git-ignored, 4–20 MiB each,
centre files for every generation). Total campaign wall: 13.6 minutes across the three,
run sequentially.

The `mean` aggregate is one new field on the ES protocol (`Aggregate {min, mean}`,
`es-train --aggregate`). `min` is not serialized, so every existing protocol hash is
unchanged; the min64 run's first 16 centre scores are byte-identical to R2b's, which
is the determinism check that makes the comparison honest.

## 2. Centre trajectories

Score = survival aggregate + `0.25 · mean normalized terminal stores`; for the `min`
runs every centre below 36,000 died on its worst layout, so the score is that
layout's survival in ticks (20 ticks = 1 s). The no-intake control dies at 7,420.

**min64** (centre score by generation; R2b is generations 0–16):

| Gens | Scores |
| --- | --- |
| 0–16 | 6,521 → 8,044 (gen 11) → 7,418: identical to R2b |
| 17–32 | 7,343, 7,262, 7,546, 7,691, 7,573, 8,119, 7,300, 7,637, 7,513, 7,593, 8,046, 7,626, 9,053, 8,362, 8,108, 8,921 |
| 33–48 | 8,041, 9,748, 8,348, 9,403, 8,797, 9,983, 11,161, 9,846, 8,630, 8,692, 8,609, 11,878, 12,737, 11,378, 9,092, 9,238 |
| 49–57 | 11,973, 14,630, 17,907, 22,466, 21,738, 22,869, 25,229, 24,989, 34,411 |
| 58–64 | **36,000** on every layout, stores tiebreak 0.14–0.18 |

The candidate median crossed the stationary-grazing controls (9,404–11,873) at
generation 44 and hit 36,000 at generation 59. First centre generation surviving the
horizon per layout: t1-corridor 52, t3-scatter 52, t2-weak-open 57, t4-ring 58. The
weak opening (fill 0.30) was solved last, as designed.

**mean16** (mean survival): 7,562, 7,221, 10,797, 13,673, 17,884, 17,567, **31,358**
(gen 6), 32,223, 32,171, 32,091, 31,997, 31,911, 32,004, 32,047, 32,168, 32,268,
32,321. From generation 6 the centre survives t1, t3 and t4 at the horizon and dies
on t2-weak-open at 17,000–21,000 ticks, and it never changes after that: the mean
gives it no reason to.

**pairs32**: 6,521, 6,499, 6,825, 7,258, 6,907, 7,307, 7,697 (best, gen 6), 6,900,
7,013, 7,136, 7,310, 6,676, 7,686, 6,889, 7,295, 7,072, 7,162. Two of 4,096
perturbed candidates survived the horizon, as in R2b. Doubling the population halves
the per-candidate step count for the same budget and, at this noise level, buys
nothing in 16 updates.

## 3. What the two foragers do (training layouts)

Costs are billed prices from resolved motion, lower bounds where marked `≥`
(`motion_billed_partial` is true on every episode: seam ticks and the death tick omit
turning). Intake is material actually removed by the mouth. Stores start at 2.5 e of
4.0 e capacity; upkeep over the full horizon is 11.16 e.

**min64, generation 59** (the frozen selection: earliest centre at 36,000 on all four,
highest score 36,000.18, file `runs/es-r2c-min64/centers/center-00059.json`, weights
FNV-1a `15300230604599313182`):

| Layout | Ticks | Stores | P / F / D (m) | Motion ≥ (e) | Travel (BL) | Cells | Ticks in opening |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| t1-corridor | 36,000 | 3.000 | 4.44 / 2.91 / 0.78 | 1.76 | 1,595 | 63 | 51 |
| t2-weak-open | 36,000 | 3.000 | 5.33 / 2.15 / 0.87 | 1.77 | 1,601 | 61 | 51 |
| t3-scatter | 36,000 | 3.000 | 5.26 / 2.49 / 0.65 | 1.80 | 1,592 | 67 | 51 |
| t4-ring | 36,000 | 2.522 | 5.70 / 1.56 / 0.98 | 1.79 | 1,609 | 49 | 72 |

It leaves the opening patch within about 2.5 s on every layout, travels roughly 1,600
body lengths over 60 cells, and eats 7–9 m of food per episode against the disclosed
scripted mobile grazer's 8.2–8.5 m of producer alone. Three of four end at the store
ceiling. Motion is 14% of the bill.

**mean16, generation 16** (final centre, file `runs/es-r2c-mean16/centers/center-00016.json`,
weights FNV-1a `1768867250715009511`), re-evaluated with `es-evaluate --set training`:

| Layout | Ticks | Stores | P / F / D (m) | Travel (BL) | Cells | Ticks in opening |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| t1-corridor | 36,000 | 1.292 | 6.74 / 0.84 / 0.67 | 1,892 | 17 | ≈ 35,700 |
| t2-weak-open | 21,284 | 0 | 3.73 / 0.00 / 0.21 | 1,117 | 18 | ≈ 20,500 |
| t3-scatter | 36,000 | 2.978 | 6.85 / 1.48 / 0.67 | 1,889 | 16 | ≈ 35,800 |
| t4-ring | 36,000 | 2.253 | 7.67 / 0.63 / 0.62 | 1,896 | 18 | ≈ 35,400 |

A different strategy: it stays in the opening patch for essentially the whole episode,
circling 16–18 cells at full pace, and lives off that patch's regrowth. That works
where the opening is rich and fails where it is weak, which is exactly the relocation
task t2 was built to pose. The mean objective rewarded solving three layouts well
over solving four.

## 4. Held-out check (frozen selection, one episode per layout)

Selection was frozen before any held-out episode ran: min64 generation 59 as the
primary, mean16 generation 16 as the secondary. The eight held-out layouts were
constructed and hashed in R2a and had never been run against a policy. Evaluation via
the new `es-evaluate` command (same episode driver, 36,000 ticks, no optimizer work),
outputs in `runs/es-r2c-eval/`.

| Layout | Opening fill | min64 g59 ticks | stores | intake (m) | mean16 g16 ticks | stores |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| h1 | 0.52 | 36,000 | 3.000 | 8.3 | 36,000 | 2.204 |
| h2 | 0.36 | **6,501** | 0 | 0.02 | 35,834 | 0 |
| h3 | 0.73 | 36,000 | 2.009 | 7.9 | 36,000 | 2.995 |
| h4 | 0.59 | 36,000 | 2.998 | 8.7 | **12,384** | 0 |
| h5 | 0.55 | 36,000 | 2.999 | 8.5 | 35,638 | 0 |
| h6 | 0.46 | 36,000 | 3.000 | 8.3 | 36,000 | 2.835 |
| h7 | 0.56 | 36,000 | 2.311 | 8.4 | 36,000 | 2.177 |
| h8 | 0.70 | 36,000 | 2.124 | 7.9 | 36,000 | 1.435 |
| **Survived** | | **7 / 8** | | | **5 / 8** | |

The min64 forager's one failure is total: on h2 it ate 0.02 m and died at 6,501
ticks, faster than a motionless body, so it never found food at all rather than
finding too little. h2 is the weakest held-out opening and its start heading points
away from the patch. The mean16 policy's two near-misses (35,834 and 35,638 ticks)
are patch exhaustion just before the horizon, the signature of its stay-put strategy.

Wall for all three evaluations: 37 s. No layout was rerun, no seed varied.

## 5. What this does and does not establish

- **A recurrent GRU policy trained by antithetic ES in the real core forages**, on
  the training layouts and on seven of eight unseen ones, under the fixed body,
  fixed ecology, the repaired starvation rule and paid motion. Nothing in the world
  was changed to make this happen.
- **The minimum-over-layouts objective was right and R2b's budget was wrong.** The
  step-function region lasts about 40 updates at this population; once one layout is
  solved the others follow within ten. The mean objective is faster but selects a
  strategy that does not generalize to weak openings.
- **Not established:** memory use (no hidden-reset diagnostic yet), behaviour past
  1,800 s, behaviour with other animals present (the shared-arena check), robustness
  across training seeds (one seed), and anything about reproduction, apex or the live
  world. The policy has never been attached to the display and was not.
- **Selection is honest:** frozen before the held-out run, training results only, one
  held-out episode per layout, no reruns.

## 6. Next scientific question

The forager exists; the question is now what it is doing. In order of value per
episode: (1) the hidden-reset diagnostic on min64 g59 (same eight layouts, hidden
state zeroed every controller tick) to see whether its foraging depends on memory;
(2) one longer-horizon run per layout (108,000 ticks) to see whether it survives
patch cycling; (3) the four-copies shared-arena check from the R2a brief. Each is
under a minute of compute at 20 workers. A second training seed would tell us whether
generation 58 is typical or lucky and costs about 8 minutes.

## Usage

Fable executed directly; no agents spawned. Billed usage unavailable. Campaign wall
13.6 min plus 37 s of evaluation.
