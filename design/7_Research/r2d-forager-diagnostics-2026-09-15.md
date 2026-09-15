---
design_status: exploration
last_reviewed: 2026-09-15
---

# R2d: what the first trained forager actually is

Revised 2026-09-15 after Astra's targeted evidence review
(`r2d-forager-review-2026-09-15.md`): cohort counts are now labelled as censored with
stopping ticks, the body-presence inputs are acknowledged, mechanism and lifespan
readings are marked as hypotheses, and seed 1's first success is corrected to
generation 58. Measurements are unchanged; no new episodes were run for the revision.

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

## 1. Hidden-state reset: intact recurrent state is load-bearing

| reset cadence | training (4) survived / min / mean ticks | held-out (8) survived / min / mean |
| --- | --- | --- |
| never (plain) | 4 / 36,000 / 36,000 | 7 / 6,501 / 32,313 |
| every 200 ticks (10 s) | 0 / 9,942 / 10,335 | 0 / 8,032 / 9,958 |
| every 20 ticks (1 s) | 0 / 6,614 / 6,848 | 0 / 6,601 / 6,665 |

With a reset every 10 s the animal eats almost nothing (producer intake 0.17–0.76 m
against 2.5–4.1 m of upkeep), visits three to four times as many cells as the plain run
(180–250 against 32–67), and starves at the reserve-only limit of about 6,500 ticks plus
whatever it grazed. A reset every second is indistinguishable from no intake at all.

Reading (hypothesis, not an identified mechanism): a periodic reset disrupts everything
the hidden state carries at once — any startup transient from the zero state, any
accumulated sensory history, any navigation state — and aggregate intake and visited-cell
counts cannot tell those apart. What is established is that this policy's intact
recurrent state is important under these conditions, and that the post-reset behaviour
is a walk rather than grazing. One consistent story is that from zero state the policy
walks first (how it leaves a weak opening) and settles into grazing only after some
seconds of built-up state, so a reset replays the walk; that is plausible and untested.
It also does not follow that a newborn will behave like a reset adult: a child's body,
reserves and surroundings differ from the training adult's, and fresh hidden state is
only one of those differences.

## 2. Four copies in one arena: censored cohort counts

The episode driver stops when the **focal** animal dies or at the horizon
(`episode.rs`), so the other copies' fates are known only up to that stopping tick; a
copy alive at the stop is censored, not a survivor. Movement, stores and upkeep in these
rows describe the focal animal; only intake is world-wide, and only to the stop.

| layout | stop tick | focal | other copies alive at the stop | copy deaths before the stop (tick) |
| --- | --- | --- | --- | --- |
| t1-corridor | 36,000 (horizon) | alive | 1 of 3 | 6,683; 28,145 |
| t2-weak-open | 22,060 (focal death) | dead | 2 of 3, censored | 6,950 |
| t3-scatter | 36,000 (horizon) | alive | 3 of 3 | none |
| t4-ring | 20,052 (focal death) | dead | 1 of 3, censored | 17,882; 18,839 |
| h1 | 36,000 (horizon) | alive | 0 of 3 | 6,440; 6,801; 14,863 |
| h2 | 6,515 (focal death) | dead | 1 of 3, censored | 6,462; 6,511 |
| h3 | 18,759 (focal death) | dead | 3 of 3, censored | none |
| h4 | 14,239 (focal death) | dead | 0 of 3 | 12,116; 12,543; 12,970 |
| h5 | 18,162 (focal death) | dead | 1 of 3, censored | 16,888; 17,189 |
| h6 | 36,000 (horizon) | alive | 0 of 3 | 6,457; 21,859; 22,081 |
| h7 | 36,000 (horizon) | alive | 0 of 3 | 16,546; 20,331; 29,968 |
| h8 | 21,841 (focal death) | dead | 3 of 3, censored | none |

Focal survival with three copies present: 2 of 4 training (t1, t3), 3 of 8 held-out (h1,
h6, h7), against 4 of 4 and 7 of 8 alone. Uncensored cohort outcomes exist only where
the focal reached the horizon: on t3 all four lived; on t1 two of four; on h1, h6 and h7
the focal alone. Deaths cluster in some arenas (h4: four within 2,100 ticks), and
world-wide producer intake with four animals runs two to four times the single-animal
figure to the same tick (t3: 21.2 m against 5.4 m), so the group does graze.

What this does and does not show: the policy **does** sense nearby bodies (the
observation layout carries six sectors of body presence and relative size plus a crowd
vector, `neural/obs.rs`; no species or role label). Those inputs were uninformative
during solitary training, so any response to crowding is unselected and unvalidated;
and the fixtures' food budget for four animals is likewise unvalidated. The rows above
measure the combination of an untested crowding response and an unknown four-animal
budget. They do not isolate a sensory deficiency, show a failure to yield, or make
cooperation necessary; dispersal and competition are both competent population
behaviours. Nor is a cube face a resource boundary: competition depends on overlapping
foraging areas over time, so "one per face" says nothing on its own.

## 3. Horizon 108,000 ticks (90 min): survival is horizon-bound

| set | survived to 108,000 | death ticks |
| --- | --- | --- |
| training | 1 of 4 (t1-corridor, stores 2.997) | t2 84,845; t3 99,238; t4 48,235 |
| held-out | 0 of 8 | 6,501 (h2); 44,387–84,563 for the rest |

Every episode the trainer scored ends at 36,000 ticks, and the objective saturates
there, so nothing selected for what happens afterwards. The animal keeps grazing (h6
takes 13.9 m of producer over 84,563 ticks against 5.1 m in the 36,000-tick run) but
does not keep pace with upkeep once the painted patches are down to regrowth. These
failures do not separate resource insufficiency from policy failure: initial stocks,
accessibility, travel cost and repeated relocation all enter, and no paid control has
shown that survival to 108,000 ticks is feasible on these layouts at all. Aggregate
regrowth exceeding upkeep would not by itself show sustained foraging either. They also
establish nothing about lifespan in the live world, whose food is not these fixtures.

## 4. A second training seed: the long plateau was partly luck

`es-train --generations 64 --workers 20 --wall-seconds 1200 --horizon 36000
--train-seed 20260916 --out runs/es-r2c-min64-seed2` (protocol hash
`0x1084d36d8675d8ce`, differing from seed 1 only in the train seed).

| update | seed 1 (20260915) centre | seed 2 (20260916) centre |
| --- | --- | --- |
| 12 | below 36,000 | 20,014 |
| 19 | below 36,000 | 36,000.064 (all four layouts) |
| 20 | below 36,000 | 36,000.152 |
| 58 | 36,000.143 (first at 36,000 on all four) | — |
| 59 | 36,000.180 (best; the R2c selection) | — |

Seed 2 clears the step at update 19 rather than 58 (seed 1's first horizon-reaching
centre was generation 58; generation 59 was selected as the best score, not the first
success). The "about 58 updates" figure in the R2c note was one draw. Final seed-2
results and a held-out check are in the addendum.

## What this changes

- Memory: this policy's intact recurrent state is important, and the reset behaviour is
  a walk. Which of startup transient, sensory integration or navigation history it
  carries is open. Do not prescribe a startup heuristic or change the sensory contract on
  this evidence.
- Population: the single-animal objective is not a proxy for a cohort, but these probes
  do not say what is missing. A future cohort experiment needs the evaluator to run to
  the horizon or the last death with each individual's death time kept separately from
  the focal's, and a food budget validated for the cohort size.
- Horizon: before training longer or changing regrowth, establish with a paid mobile
  control that survival over the longer duration is feasible on the layout, and pair it
  with stationary grazing to show relocation is necessary. Do not make the task solvable
  by raising local regeneration.
- Selection: the earliest horizon-reaching centre is the wrong freezing rule (addendum).
  The held-out layouts have now been inspected against two seeds and are a diagnostic
  set; a later generalisation claim needs a fresh untouched test set.

## Addendum: seed 2 completed

64 updates, 8,452 episodes, 251.1 M ticks, 1,064.6 s wall at 20 workers
(`runs/es-r2c-min64-seed2/`). Centre scores: first at 36,000 on all four at update 19
(36,000.064); highest at update 64 (36,000.184), rising monotonically through the
plateau as the store term improves. Elapsed: 1,064.6 s wall (from `runs/es-r2c-min64-seed2.log`). Selection was frozen
before any held-out episode: both the first horizon-reaching centre (19) and the highest (64), evaluated on the eight held-out
layouts (`runs/es-r2c-diag/seed2-g{19,64}-holdout.{txt,json}`):

| centre | held-out survived | min ticks | mean ticks | fails |
| --- | --- | --- | --- | --- |
| seed 1, g59 (R2c; best score, first success was g58) | 7 of 8 | 6,501 | 32,313 | h2 (ate 0.02 m) |
| seed 2, g19 | 5 of 8 | 13,475 | 27,916 | h2, h4, h5 |
| seed 2, g64 | 7 of 8 | 14,475 | 33,309 | h2 (ate 1.50 m, died at 14,475) |

Two independent seeds fail the same held-out layout, h2 (the weakest opening with the
start heading away from the patch), and pass the other seven. The first centre to clear
the training step transfers worse than the same run's later plateau centres, so "earliest
at 36,000" is not the right freezing rule; the store term keeps carrying information once
survival saturates. (One comparison on one seed pair; a rule change should be stated in
the next training protocol, not inferred from this.) Seed 2's g64 forager has slightly higher stores and lower path
length (about 1,420 body lengths against 1,600) than seed 1's g59: it walks less to eat
the same.
