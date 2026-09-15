---
design_status: exploration
last_reviewed: 2026-09-15
---

# R2d: what the first trained forager actually is

Four diagnostics on the R2c selection (min64 generation 59,
`runs/es-r2c-min64/centers/center-00059.json`, weights FNV-1a
15300230604599313182), plus a second training seed. Each ran through the same episode
driver the trainer uses (`es-evaluate`, build `cb39bce` plus the probe options below).
Outputs under `runs/es-r2c-diag/` (git-ignored). No optimizer work; nothing in
`runs/es-r2c-min64` changed. The plain held-out rollout was rerun first and every
per-episode number is identical to `runs/es-r2c-eval/min64-g59-holdout.json`, so the
probe path (`episode::run_with_fault` with a hook that does nothing) is the plain path.

New `es-evaluate` options, both diagnostics and never training conditions (the protocol
hash does not see them): `--reset-hidden-every <ticks>` zeroes every animal's GRU hidden
state on that cadence, keeping the held action and feedback; `--copies <n>` places n − 1
exact copies of the animal (same body, same reserve, fresh hidden state, same
`bud = false` script) in the opening patch's other cells at tick 0 and records when each
dies.

## 1. Hidden-state reset: the forager is a startup programme, not a reflex

| reset cadence | training (4) survived / min / mean ticks | held-out (8) survived / min / mean |
| --- | --- | --- |
| never (plain) | 4 / 36,000 / 36,000 | 7 / 6,501 / 32,313 |
| every 200 ticks (10 s) | 0 / 9,942 / 10,335 | 0 / 8,032 / 9,958 |
| every 20 ticks (1 s) | 0 / 6,614 / 6,848 | 0 / 6,601 / 6,665 |

With a reset every 10 s the animal eats almost nothing (producer intake 0.17–0.76 m
against 2.5–4.1 m of upkeep), visits three to four times as many cells as the plain run
(180–250 against 32–67), and starves at the reserve-only limit of about 6,500 ticks plus
whatever it grazed. A reset every second is indistinguishable from no intake at all.

Reading: from the zero hidden state the policy first walks (this is how it leaves a
weak opening and finds the next patch; the trainer rewarded exactly that), and only
after some seconds of accumulated state does it settle into grazing. A reset replays the
walk forever. So the recurrence is load-bearing, but what it carries is mostly a clock
and a mode, not a map. This matters for the display: a child is born with zero hidden
state and will do the same startup walk.

## 2. Four copies in one arena: the trained forager does not share

| set | focal survived | copies alive at the end (per layout) |
| --- | --- | --- |
| training | 2 of 4 (t1, t3) | 2, 2, 4, 1 of 4 |
| held-out | 3 of 8 (h1, h6, h7) | 1, 1, 3, 0, 1, 1, 1, 3 of 4 |

Deaths cluster: on h4 all four die within 2,100 ticks of each other (12,116–14,239); on
h2 three die within 60 ticks of the reserve limit. Total producer intake with four
animals is two to four times the single-animal figure (t3: 21.2 m against 5.4 m for the
plain run on comparable layouts), so the group does graze, but the patches are sized for
one and the policy has no conspecific input, so it neither spreads out nor yields. On
the cube, four seeded copies on four different faces do not compete at first; two on one
face would.

## 3. Horizon 108,000 ticks (90 min): survival is horizon-bound

| set | survived to 108,000 | death ticks |
| --- | --- | --- |
| training | 1 of 4 (t1-corridor, stores 2.997) | t2 84,845; t3 99,238; t4 48,235 |
| held-out | 0 of 8 | 6,501 (h2); 44,387–84,563 for the rest |

Every episode the trainer scored ends at 36,000 ticks, and the objective saturates
there, so nothing selected for what happens afterwards. The animal keeps grazing (h6
takes 13.9 m of producer over 84,563 ticks against 5.1 m in the 36,000-tick run) but
does not keep pace with upkeep once the painted patches are down to regrowth. Whether
that is the policy or the layouts' food budget is not separated here; a layout with
a regrowing field larger than one animal's upkeep would separate it. For the display,
the honest expectation is that a seeded forager lives on the order of an hour of world
time, not indefinitely, unless the live world's food is richer than the fixtures.

## 4. A second training seed: the long plateau was partly luck

`es-train --generations 64 --workers 20 --wall-seconds 1200 --horizon 36000
--train-seed 20260916 --out runs/es-r2c-min64-seed2` (protocol hash
`0x1084d36d8675d8ce`, differing from seed 1 only in the train seed).

| update | seed 1 (20260915) centre | seed 2 (20260916) centre |
| --- | --- | --- |
| 12 | below 36,000 | 20,014 |
| 19 | below 36,000 | 36,000.064 (all four layouts) |
| 20 | below 36,000 | 36,000.152 |
| 59 | 36,000.18 (first at 36,000 on all four) | — |

Seed 2 clears the step at update 19 rather than 59. The "about 58 updates" figure in
the R2c note was one draw. Final seed-2 results and its held-out check: see the
addendum below when the run completes.

## What this changes

- The R2c plan's "hidden-reset diagnostic" question is answered: memory matters, and in
  a specific way (startup transient). Any objective that starts animals mid-patch with
  nonzero state, or any display expectation of "born and immediately grazes", is wrong.
- A shared-arena objective (several animals per layout) is needed before the trained
  forager is meaningful at population scale; the single-animal one is not a proxy.
- Horizon: either train at the horizon we care about, or add a regrowth-sufficient
  layout so that 36,000-tick survival implies sustained foraging.

## Addendum: seed 2 completed

64 updates, 8,452 episodes, 251.1 M ticks, 1,064.6 s wall at 20 workers
(`runs/es-r2c-min64-seed2/`). Centre scores: first at 36,000 on all four at update 19
(36,000.064); highest at update 64 (36,000.184), rising monotonically through the
plateau as the store term improves. Selection was frozen before any held-out episode:
both the R2c rule's centre (19) and the highest (64), evaluated on the eight held-out
layouts (`runs/es-r2c-diag/seed2-g{19,64}-holdout.{txt,json}`):

| centre | held-out survived | min ticks | mean ticks | fails |
| --- | --- | --- | --- | --- |
| seed 1, g59 (R2c) | 7 of 8 | 6,501 | 32,313 | h2 (ate 0.02 m) |
| seed 2, g19 | 5 of 8 | 13,475 | 27,916 | h2, h4, h5 |
| seed 2, g64 | 7 of 8 | 14,475 | 33,309 | h2 (ate 1.50 m, died at 14,475) |

Two independent seeds fail the same held-out layout, h2 (the weakest opening with the
start heading away from the patch), and pass the other seven. The first centre to clear
the training step transfers worse than the same run's later plateau centres, so "earliest
at 36,000" is not the right freezing rule; the store term keeps carrying information once
survival saturates. Seed 2's g64 forager has slightly higher stores and lower path
length (about 1,420 body lengths against 1,600) than seed 1's g59: it walks less to eat
the same.
