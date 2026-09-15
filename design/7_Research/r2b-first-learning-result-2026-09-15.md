---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# R2b: first bounded learning screen — 2026-09-15

Executed by Fable under the [R2b brief](../handoffs/r2b-fable-first-learning-2026-09-15.md)
against the R2a trainer verified at `dbb769e`
([review](r2a-trainer-review-2026-09-15.md)). This is evidence, not a decision. It
answers one question: does the first fixed-budget antithetic-ES screen improve the
central GRU policy's survival on the four validated foraging layouts?

**Short answer: barely.** The centre moved from dying *faster* than a motionless body
to dying at about the same time as one. It does not forage. Two of 2,048 perturbed
candidates survived the whole horizon on one layout, so the capability exists near
the centre; the centre never captured it.

## 1. Exactly what ran

| Item | Value |
| --- | --- |
| Source | `dbb769e` (working tree carried unrelated uncommitted files outside `crates/`, hence the `-dirty` stamp) |
| Build stamp | `dbb769eab5d2-dirty` |
| Protocol hash | `0xb69033f65e56f1df` (matches the brief) |
| Policy schema digest | `0xb409fed56734b25e` |
| Output | `runs/es-first-repaired/` (checkpoint, `generations.jsonl`, 17 centre files; 4.6 MiB; `runs/` is git-ignored, so the artefacts are local) |
| Workers | 16, on Wrysk's instruction during launch (the brief said 8; scores and updates are worker-count independent, verified in R2a) |
| Start / end | 2026-09-15 03:10:36 / 03:11:45 PDT |

```
cargo run -p cubarium-search --release -- \
    es-train --pairs 16 --generations 16 --horizon 36000 \
             --workers 16 --wall-seconds 1200 --train-seed 20260915 \
             --center-eval true --out runs/es-first-repaired
```

An 8-worker launch of the same command was stopped within seconds of starting and its
seconds-old directory deleted before the 16-worker run; no partial run was resumed
and no budget was consumed by it beyond those seconds.

**Work.** 16 updates completed. 2,116 episodes: 2,048 perturbations + 68 centre
evaluations (17 centres × 4 layouts). 16,342,111 ticks simulated of the 76,176,000
scheduled maximum, because almost every episode ended in death well before the
36,000-tick horizon. Discarded work: 0 episodes, 0 ticks. Wall: **68.7 s** of the
1,200 s cap, about 4.2 s per generation.

**Pre-repair run.** `runs/es-first` (build `80bf7186d98c-dirty`, same seed and
protocol hash) recorded the identical 17 centre scores. That run is therefore not
an independent replicate; it is the same deterministic trajectory produced before
the repairs, without centre files or complete diagnostics. It is preserved
untouched.

## 2. Centre trajectory

Score = `t_min + 0.25 · mean(clipped terminal stores)`; every centre died on every
layout, so each score is exactly its worst-layout survival in ticks (20 ticks = 1 s).

| Gen | Centre score | Gen | Centre score |
| ---: | ---: | ---: | ---: |
| 0 | 6,521 | 9 | 7,510 |
| 1 | 6,532 | 10 | 7,572 |
| 2 | 6,657 | **11** | **8,044** |
| 3 | 7,146 | 12 | 7,478 |
| 4 | 7,572 | 13 | 7,573 |
| 5 | 7,261 | 14 | 7,308 |
| 6 | 7,395 | 15 | 7,988 |
| 7 | 7,260 | 16 (final) | 7,418 |
| 8 | 7,135 | | |

Reference points from the R2a controls on the same layouts: a body that never eats
dies at **7,420** ticks; stationary continuous grazing dies at 9,404–11,873; the
disclosed paid mobile script survives all 36,000 funded.

- Initial centre (6,521): 12% *shorter* than doing nothing.
- Best recorded centre, generation 11 (8,044): 8% longer than doing nothing, and
  shorter than standing still and chewing on every layout.
- Final centre (7,418): indistinguishable from doing nothing.

The trajectory is non-monotonic after generation 4 and the Adam update RMS decays
from 0.010 to 0.003, which reads as a noise-dominated gradient estimate on a
step-shaped objective rather than steady ascent.

## 3. What the centres actually did (per layout)

All values from the campaign's own episode records; costs are **billed prices**
reconstructed from resolved motion, not ledger payments, and `motion_billed_partial`
is true on every episode (seam-crossing ticks and the death tick omit turning), so
motion and sweep are **lower bounds**. Intake is material actually removed by the
mouth (settlement diagnostics). Stores start at 2.5 e of a 4.0 e capacity.

**Generation 0 (initial centre)**

| Layout | Ticks | Intake P / F / D (m) | Upkeep (e) | Motion ≥ (e) | Travel (BL) | Cells | Sweep ≥ (rad) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| t1-corridor | 6,521 | 0.030 / 0.003 / 0.000 | 2.02 | 0.33 | 335 | 144 | 53.9 |
| t2-weak-open | 7,470 | 0.124 / 0.091 / 0.022 | 2.32 | 0.37 | 383 | 208 | 58.8 |
| t3-scatter | 7,870 | 0.188 / 0.116 / 0.026 | 2.44 | 0.39 | 404 | 195 | 61.8 |
| t4-ring | 8,389 | 0.247 / 0.161 / 0.036 | 2.60 | 0.42 | 431 | 142 | 67.6 |

**Generation 11 (selected centre)**

| Layout | Ticks | Intake P / F / D (m) | Upkeep (e) | Motion ≥ (e) | Travel (BL) | Cells | Sweep ≥ (rad) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| t1-corridor | 8,520 | 0.242 / 0.158 / 0.034 | 2.64 | 0.36 | 398 | 249 | 12.9 |
| t2-weak-open | 8,044 | 0.196 / 0.115 / 0.026 | 2.49 | 0.35 | 378 | 234 | 19.5 |
| t3-scatter | 8,151 | 0.184 / 0.134 / 0.036 | 2.53 | 0.36 | 385 | 227 | 20.2 |
| t4-ring | 8,200 | 0.213 / 0.135 / 0.024 | 2.54 | 0.36 | 386 | 261 | 17.3 |

**Generation 15 (last scored before the final)**: 7,988 / 8,610 / 11,004 / 8,159
ticks; the t3-scatter episode ate 0.95 m and is the only centre episode that beat
the stationary-grazing control on its layout. The final centre (generation 16,
score 7,418) has no per-layout record in the generation log beyond its score.

Reading: the initial policy wanders about 400 body lengths at full pace, spinning
(50–70 rad of sweep), and eats almost nothing. Training reduced the spinning by a
factor of three to four and roughly doubled intake, but intake stayed at about
0.2 m of producer over a lifetime, against the 0.8–1.0 m a stationary chewer
takes and the 8.5 m the scripted mobile grazer takes. Upkeep, not motion, is
what kills every centre: motion is about 13% of the bill.

## 4. The candidates

| Gen | Best candidate | Median | Worst | Gen | Best | Median | Worst |
| ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 0 | 8,084 | 6,900 | 6,476 | 8 | 7,871 | 7,137 | 6,575 |
| 1 | 8,117 | 6,934 | 6,432 | 9 | 8,092 | 7,132 | 6,537 |
| 2 | 7,919 | 6,893 | 6,460 | 10 | 7,689 | 7,181 | 6,581 |
| 3 | 7,925 | 7,028 | 6,450 | 11 | 7,961 | 7,250 | 6,474 |
| 4 | 7,929 | 7,092 | 6,507 | 12 | 7,729 | 7,060 | 6,485 |
| 5 | 7,613 | 6,988 | 6,555 | 13 | 7,951 | 7,145 | 6,592 |
| 6 | 7,724 | 7,164 | 6,560 | 14 | 8,085 | 6,979 | 6,505 |
| 7 | 7,649 | 7,261 | 6,577 | 15 | 8,295 | 7,273 | 6,592 |

The candidate median rose from 6,900 to about 7,250 over the screen, the same
modest drift as the centre. **Two of 2,048 perturbation episodes survived the
36,000-tick horizon**, both on t1-corridor; the longest (generation 6, `pair2−`)
finished with 0.69 e of stores after eating 4.08 m of producer, 1.76 m of fruit and
1.05 m of detritus, travelling 1,649 body lengths across 138 cells. That candidate's
score is not a centre score and it died on its other layouts, so `t_min` gave it no
credit. A perturbed candidate's score is never reported here as a result.

## 5. Selected artefact

Highest recorded centre score, ties to the earliest generation: **generation 11**,
score 8,044, file `runs/es-first-repaired/centers/center-00011.json`, weights FNV-1a
`4074601552855445607`. The final centre is generation 16, score 7,418, file
`centers/center-00016.json`, weights FNV-1a `13206531044472264029`; it is scored,
and it is not the winner. Neither has been attached to any live world, evaluated on
the held-out layouts, or run past 36,000 ticks.

## 6. Answer and the next scientific question

**Did the screen improve survival on the training layouts?** Slightly: +1,523 ticks
(76 s) on the best centre and +897 on the final, from a start that was worse than
inaction. It did not produce a forager, and no centre beat the stationary control
on more than one layout. This is a result about *this configuration* (16 pairs,
16 updates, sigma 0.02, `t_min` over four layouts on a survival objective), not
evidence that recurrent control cannot learn to forage.

**What the data says about why.** The objective is nearly a step function in the
region the centre occupies: every candidate dies between 6,400 and 8,300 ticks, so
ranks are decided by tens of seconds of upkeep, and the one behaviour that matters
(finding and eating enough to change the survival time by thousands of ticks) shows
up in two candidates out of 2,048 and is invisible to a minimum over four layouts.
The gradient norm stays flat at roughly 250 throughout, consistent with rank noise
rather than a consistent direction.

**Next question, for a separately specified brief, not run here:** with the same
frozen optimizer and fixtures, does the signal become usable when (a) the campaign
runs longer at the same population, (b) the population is larger at the same
budget, or (c) the layouts are scored with a mean rather than a minimum so a partial
forager gets credit? Option (c) changes the frozen score convention and is a
decision, not a tuning knob; (a) and (b) are budget decisions. The held-out
evaluation, hidden-reset diagnostic and shared-arena transfer check reserved by the
R2a brief still need their own budget before any of them runs, and with no centre
that forages there is little to evaluate yet.

## Usage

Fable executed this directly; no agents were spawned under this assignment.
Billed token usage is unavailable in this harness. Wall time for the campaign
itself: 68.7 s.
