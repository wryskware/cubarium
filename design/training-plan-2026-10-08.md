---
design_status: proposal
last_reviewed: 2026-10-08
decision_refs: []
---

# Training plan: brains and ecology (2026-10-08)

Wrysk asked (2026-10-08) for a diagnosis, a fitness score "we" define together, an efficient
protocol for runs and feedback, d72/d96 or flat-zone maps, and the spare compute used while
he's away. This merges two independent diagnoses and states what is running. Nothing here is
decided; §6 lists his calls.

- Opus: `design/7_Research/training-readiness-2026-10-08.md`
- Astra: `design/7_Research/astra-training-readiness-2026-10-08.md`
- Brain contract v3 proposal: `design/brain-contract-v3-proposal-2026-10-08.md`

## 1. Where we are

**Brains.** Only frondgrazer and littershredder have brains (voxel manifest v2, GRU 32,
3 actions). Both are the 10-02 C1 centres, built in and on by default in the live world and in
`voxel_census`. They were trained before the gut, the Veilhound packs, dark-active shredders,
succession and the new senses. A GRU body skips the reflex layer, so it never flees or wades
out. It also can't sprint, and it sees none of the S1–S3 senses. The other seven lineages are
refused ("no GRU policy until contract v3").

**Search and training tools.** They build. Three `cubarium-search` tests fail on one cause
(the seedporter now eats carrion); a fix is on branch `search-diet`.

The scoring misleads in several ways (Astra):
- The roster search leaves Veilhound, lanternjaw and loftstrider out of every penalty.
- It forgives a plant loss when the default lost the same plant on that seed.
- Burn-in losses are dropped from persistence.
- Births are sampled once a minute.
- The brain reward ignores the stomach.

The search and trainer loops have no staged checkpoints or liveness checks.

**Founding (fixed, 110cb418).** Founders arrived with their stomach at the appetite floor, so
the Veilhound pack hunted at t=0: 1–8 founders died in minute 1 of every run, and the
loftstriders were lost in 4 of 14. Founders now arrive fed (`founding.arrival_stomach` 1.0),
and minute-1 kills went from 1–3 to 0 on four seeds. The staged check now accepts sated
grazers (c7603d50).

**Throughput.** A freshly founded d72 world runs at 1.8 ms/tick (27× real time, 4 threads).
By minute 60 the baseline's 1-thread worlds, 16 at a time on eidolon, slowed to 18–46 ms/tick,
which is 2–4× real time:
- At d72 minute 88: water 8.2 ms, plants 5.7 (drinking 2.9), animals 4.4 (senses 3.5).
- d96 seed 1 spends 30+ ms/tick in the animal phase outside sensing from minute 4 on: a
  pathology.

The SIM-SPEED package is on it. At today's speed, an 8-seed × 2-arm × 2 h check takes about
40 min on eidolon, against the 30-min target.

### 1a. Baseline v2 (before today's heuristic bug fixes)

Setup: main 691d3f8e, d72, saved starts at minute 63 (seeds 1–8), 2 h each. Arms: heuristic
vs the shipped C1 brains. Scored with `score.py --profile screen-2h-legacy` (report:
`runs/bl2-2026-10-08/report.md`).

**Persistence.** Both arms are feasible: no required lineage or species was lost on any
seed. Lanternjaws stay at about 1.7 % of animals (Z penalty 1). Veilhounds made about 2 kills
per world-hour.

**C1 brains against heuristics.**

| Lineage | Measure | C1 | Heuristic |
|---|---|---|---|
| Frondgrazer | alive at end | 32 | 22 |
| Frondgrazer | p10 fullness | 0.13 | 0.007 |
| Frondgrazer | intake | 2.3× | 1× |
| Frondgrazer | moving share | 0.25 | 0.57 |
| Littershredder | intake | 5.0 | 2.8 |

So the brains feed far better and move half as much. With movement weighted 10, the heuristic
arm ranks better (soft P 8.8 against 9.9; C1 wins 1/8). How much movement is worth against
condition is Wrysk's call (§6 Q5).

**Condition is poor at the bottom.** p10 fullness is near zero in every lineage except the
Veilhound. Populations are about two-thirds juveniles after the founding birth pulse; bellwings
went from 6 to 70 and lose about 5 a seed to starvation. VITALS-3 adds condition by life stage
to say whether these are starving juveniles.

**Stocks growing unchecked.**
- Litter rose by about 6,000 in 2 h. The answering role, the shredders, was frozen by bug 1
  below.
- Velvetpad rose by 46 stands/min, while springturf fell by 13/min.
- Standing foliage rose.

**Heuristic bugs found by the readouts, all fixed since** (FORAGE-LEAVE 54301f40,
FROZEN-PREY 0cd0767f):
1. Littershredders froze on any footfall, grazers' included (Flee held them 43 % of the time).
   Capgnawers ran from the same footfall 66 % of the time.
2. Browsers facing a wall wobbled their heading forever. Frondgrazers sat like that for
   20–50 min at zero reserve.
3. Seedporters nibbled empty crowns for tens of minutes.
4. The staged check called hungry frozen bodies "idle".

### 1b. Baseline v3 (fixed heuristics)

Setup: main fabfc142, start library v2 `runs/starts2-2026-10-08` (minute 63), seeds 1–8,
2 h. Report: `runs/bl3-2026-10-08/report.md`, profile `screen-2h`.

**Persistence.** Both arms are again feasible.

**C1 against heuristic.**

| Measure | C1 | Heuristic |
|---|---|---|
| Frondgrazers alive at end | 31.5 | 22 |
| Frondgrazer juvenile p10 fullness | 0.11 | 0.005 |
| Frondgrazer moving share | 0.27 | 0.56 |
| Kills per world-hour | 1.5 | 2.5 |
| Soft P | 11.1 | 10.4 |

- C1 wins 3/8. Movement (V) again decides the ranking.
- Littershredders are now alike in both arms (intake about 5.3), since the heuristic shredder
  no longer freezes.

**The bottom decile is juveniles.** Adults are fed: p10 fullness is 0.26 for the frondgrazer
and the capgnawer, 0.47 for the loftstrider and 0.16 for the littershredder. Heuristic
juveniles starve:
- capgnawer juveniles: all of them below 0.05;
- bellwing juveniles: nearly all; bellwing adults are low too;
- frondgrazer juveniles: p10 0.005.

C1 frondgrazer juveniles are fed, so food exists for a juvenile that forages well.
JUVENILE-FOOD is diagnosing.

**Stuck: mostly real, now measured as a prevalence** (STUCK-AUDIT, a67b64f8).
- The 0.9–1.0 conditional shares were mostly true. 956 of 1,230 exposed body-minutes are
  juveniles in emergency at near-zero fullness:
  - pacing in place: 100–1,800 body lengths of path for under 1 BL net in 5 min;
  - resting hungry with nothing in sense and no search;
  - motionless in the snail's roam steer and in the shredder's shallows reflex;
  - seedporters clinging to stems.
- Two false positives are fixed: a pregnant Veilhound waiting at the den for the hunt quorum,
  and digesting croppers.
- S is now stuck body-minutes over awake body-minutes: heuristic 9.5 %, C1 5.6 %.
- These are search failures, most likely the root of the juvenile starvation (JUVENILE-FOOD).

**Phase check at minute 120** (`runs/bl3b-m120-2026-10-08/report.md`, same starts loaded an
hour later, 2 h). This repeats bl3:
- C1 wins 2/8 (median Δ soft P +0.93 against C1). V again decides.
- C1 keeps more frondgrazers alive (26.5 against 20.5) and is stuck less (S 3.3 % against
  4.9 %).
- In both arms:
  - littershredders barely move (moving share about 0.01). This is a choice, not a failure
    (`design/7_Research/shredder-plateau-2026-10-08.md`):
    - a shredder that tastes litter holds still and feeds;
    - its gut sets the rate;
    - the 0.19 is a late-window average of a sawtooth that moves in step across seeds:
      adults sleep through the lit two-thirds of the day, then refill;
    - litter is decomposition-limited. Shredders eat about 0.02 % of the decay flow, and
      even 41 shredders at full gut and full size would eat only about 0.15/min, so they
      are not the litter sink at any plausible population;
    - the scorer's litter role now says so;
  - bellwing fullness is effectively zero and 4–5 starve per seed after the founding hatch
    boom (§6 Q8);
  - every snail and capgnawer juvenile is under the 0.05 starving line. That is growth by
    design (JUVENILE-FOOD), so for those lineages the starving count is not a health signal.

### 1d. Baseline v4 (TRAPS, HUNGRY-SEARCH, LEADS-PAY, JUVENILE-FOOD)

Setup: binary c92ce6e7 (before NECTAR-SCENT), start library v3 (`runs/starts3-2026-10-08`,
minute 63), seeds 1–8, 2 h. Report: `runs/bl4-2026-10-08/report.md`.

**The search and stuck failures are gone, and the heuristic now feeds about as well as C1.**

| Measure | Heuristic v3 | Heuristic v4 | C1 v4 |
|---|---|---|---|
| Frondgrazers alive at end | 22 | 40.5 | 42 |
| Frondgrazer juvenile p10 fullness | 0.005 | 0.10 | 0.14 |
| Bellwing adult p10 fullness | about 0 | 0.19 | 0.19 |
| Stuck prevalence S | 9.5 % | 0.6 % | 0.5 % |
| Frondgrazer intake | n/a | 43 | 50 |
| Frondgrazer moving share | 0.56 | **0.0075** | 0.36 |

- **V now decides everything, the wrong way.** C1 wins 8/8 with median Δ soft P −9.8, and
  −9.6 of that is V. A fed heuristic frondgrazer stands in a meadow and crops: it moves under
  1 % of the time and loses nothing for it.
- Littershredders, capgnawers and loftstriders also move about 0 % in both arms.
- Standing foliage grows by about 2,500 over 2 h, and litter by about 5,800.
- **This is Wrysk's point exactly** (WORKING_POLICY "Shape the situation"): the food is so
  abundant that moving doesn't pay, and a movement term in P then punishes the animal for
  the world's generosity. The lever is the supply: plant regrowth, patchiness and the browse
  share per bite. The 2026-09-28 parameter audit found plants about 10³× generous. That is
  the first gene group for the ecology search (§6 Q10), and V comes out of P (§6 Q5).
- Every snail and capgnawer juvenile is still below the starving line. That is growth by
  design (JUVENILE-FOOD), not hunger.

**Baseline v5** (`runs/bl5-2026-10-08`: v4 plus NECTAR-SCENT and SAVE-EXTENSIBLE, from the
resaved library) repeats v4:
- C1 wins 8/8 on V alone; stuck S is 0.5–0.7 %.
- Bellwings are more numerous at the same condition: 93.5 against 84.5 alive, 41.5 against 34
  adults, adult p10 0.18. The scent now feeds more of them.

### 1c. Brain pilot (eidolon, 09:35–14:15)

Setup:
- frondgrazer and littershredder brains, warm-started from C1;
- train-flat-nopred, σ 0.02, learning rate 0.005, 8 pairs × 8 worlds, 10-min windows;
- curiosity recalibrated (`config/training/curiosity-2026-10-08.toml`: frondgrazer 2.78,
  littershredder 0.49; the old weights were 81 and 27);
- training stopped at its wall budget after 21 generations.

Tables: `runs/brain-pilot-2026-10-08/{p1-noise,p2-train}.tsv`. Gates: `runs/pilot-mid-2026-10-08/flat`
and `runs/pilot-end-2026-10-08/{flat,d72}`, profile `gate-1h`, 8 seeds, 1 h fresh founding.

**The loop works.** Staging, the wall budget, resume, gates and scoring all ran.

**The gradient is at noise level.**
- The spread of paired differences in training is 0.16–0.62 per generation. The σ = 0
  placement-noise control gives 0.07–0.44.
- The frondgrazer's mean raw reward shows no trend over 21 generations.
- Stomach-aware energy barely changes the rankings (within-world ρ 0.93–0.96).

**Yet behaviour drifted.** Final frondgrazer brain against C1:

| Measure | Flat: trained | Flat: C1 | d72: trained | d72: C1 |
|---|---|---|---|---|
| Moving share | 0.60 | 0.33 | 0.59 | 0.47 |
| Delivered BL/s | 0.117 | 0.049 | 0.125 | 0.063 |
| Intake | 9.7 | 14.2 | 8.7 | 13.2 |
| Fullness p10 | 0.08 | 0.13 | 0.07 | 0.11 |
| Cohort energy gain | 0.150 | 0.235 | 0.128 | 0.217 |
| Stuck observations | 52 | 22 | 66 | 15 |

- Trained frondgrazers move twice as fast and eat about a third less. The midway gate sat
  between C1 and the end (intake 11.5).
- Littershredders are unchanged: intake 1.03 against 1.04, and both brains hold still on
  litter (§1b).
- P calls it a win: on flat 6/8 against C1 (median Δ −0.30); on d72 5/8 (median Δ −0.04,
  a tie). V carries it.
- The cause is either the reward's curiosity share (0.5 of the energy spread) or a
  noise-driven drift. Either way, the brain reward and P both paid for motion over food.
- The reward split points to curiosity:
  - over generations 16–21 the frondgrazer's curiosity term rose from 0.15 to 0.21 while its
    energy term fell from 0.80 to 0.70;
  - curiosity's spread across bodies is 0.15–0.23 against energy's 0.4–0.5, enough for
    the search to trade one for the other.
- **Control `p3-zero`: curiosity caused the drift.** It's the same training with curiosity 0
  (20 generations), then the same flat gate. Results are in `runs/pilot-zero-2026-10-08/`
  (`score.txt` and `report.md`, scored with `--baseline mid:c1` through
  `cmp/{zero,mid}`; same binary as the midway gate).

  | Frondgrazer, flat, median | No curiosity | Curiosity, midway | Curiosity, end | C1 |
  |---|---|---|---|---|
  | Moving share | 0.22 | 0.49 | 0.60 | 0.33 |
  | Delivered BL/s | 0.036 | 0.097 | 0.117 | 0.049 |
  | Intake | 13.6 | 11.5 | 9.7 | 14.2 |
  | Fullness p10 | 0.13 | 0.10 | 0.08 | 0.13 |
  | Cohort energy gain | 0.227 | 0.188 | 0.150 | 0.235 |

  - Without curiosity the brains feed like C1 and move a little less.
  - They don't beat C1 either:
    - generation scores swing from 0.42 to 0.96 with no trend (`gensplit.py`);
    - the noise table repeats p1 (paired-difference SD 0.35, shredder 0.38).
  - P ranks it below C1 (2/8, median Δ +1.44) only because V penalises a moving share under
    0.30.
  - Littershredders are unchanged again (intake 1.04).

**What it means:**
- **Curiosity is a reward for moving, and it bought motion at the cost of food.** Under "Shape
  the situation" (WORKING_POLICY), my recommendation is curiosity 0 for training. Exploration
  should pay through the world: food scarce or patchy enough that a body that stays put goes
  hungry (FOOD-BUDGET, Q10). Your call (Q14).
- Without curiosity the training doesn't drift, but it doesn't learn either. In this world
  every brain that eats is about as good as every other. Browsers take 0.33 % of foliage
  production, and a body breaks even feeding 2–5 % of the time (FOOD-BUDGET). So behaviour
  barely changes energy, and the gradient stays at noise level.
  More worlds (EC2) or longer windows would sharpen the estimate. A world where behaviour
  decides intake would give it something to estimate. Run the next brain pilot after the
  ecology genes make food matter.
- The V weight against F and the energy gain (§6 Q5) is now a concrete choice: as weighted, P
  promotes a brain whose bodies gain energy 36 % slower than C1's.
- The heuristic arm in these gates ran on the 09:27 binary, before today's heuristic fixes,
  so it is out of date. Baseline v4 has the current heuristics.

## 2. The fitness score

One rule from both reviews: **a vector first, a scalar second.** Every evaluation prints:
- a per-lineage and per-species health table;
- the composite, used only to rank candidates;
- the paired difference against the baseline on the same seeds and starts.

A required species lost can never be bought back by any other term.

### 2a. Brain reward (per body, per window)

`r = (E_end − E_start + G) / M_ref + w_L · N`
- `E` is the body's stored energy (body + reserve + gestation escrow). It is 0 at death,
  whatever the cause.
- `G` is organic matter given to offspring.
- `N` is curiosity: new body-length cells, with familiarity decaying at a 30-min half-life.

No bonus for bites, kills, births, speed or survival. The ecology score measures population
outcomes instead.

Routine changes, which we can make:
- score awake time only;
- rank-normalise within each world (centring exists);
- recalibrate `w_L` on today's world (`w·sd(N) = share·sd(energy)`, shares 0.5 for the
  frondgrazer and 0.3 for the littershredder);
- a σ = 0 control to measure placement noise before reading any gradient.

Wrysk's calls: whether `E` counts the digestible stomach (§6 Q3), and the reward for pack and
parental lineages (v3 Q4).

### 2b. Ecology objective (per world, minimise)

Per world:

`P = 100 L + 10 J + 5 Q + 2 A + 2 K + 10 V + 2 Z + 3 M + G_b + G_v + H + 3 F + 5 S`

Here `h(x) = clip(1 − x, 0, 1)` and `d(x; lo, hi)` is the clipped relative distance outside
the band `[lo, hi]`.

| Term | Meaning | Source | Status |
|---|---|---|---|
| L | required lineages and species lost, counted absolutely: founding and burn-in losses count, no forgiveness because the reference also lost them | evaluation end state | ACCOUNTING branch |
| J | mean reproductive deficit per lineage, over the interval where breeding was possible | exact birth events | ACCOUNTING branch |
| Q | replenishment shortfall over one day cycle, `h(I/(I − ΔS))` | `targets.rs` cycle books | exists |
| A | band: late counts within `[N0, 3N0]` per lineage | trace | exists, switched off by `counts_readout` |
| K | peak above `5 N0` | trace | exists |
| V | browser movement share below 0.30. **Proposed: drop from P, keep as a diagnostic.** Under WORKING_POLICY "Shape the situation" (Wrysk 2026-10-08), a movement term tells animals what to do; if they don't need to move to eat, the food is too abundant. Your call (Q5) | window meter | exists |
| Z | lanternjaw share outside 5–15 % of animals (full-world checks only) | trace | exists |
| M | largest meadow patch below its target area | census `meadow_patch_max` | in CSV |
| G_b, G_v | lanternberry grove size; new established vaulttree sites | census grove/forest columns | partial |
| H | hunts per world-hour outside the target band | strikes and kills by hunter × prey | reducer needed |
| **F** | **condition**: mean over lineages of `h(p10 fullness / 0.3)` (VITALS emits p10, not p25) | VITALS | new (Wrysk's vitals ask) |
| **S** | **stuck**: stuck body-minutes over awake body-minutes. A stuck body is exposed (eating or emergency latch on, no bite for a whole 5-min window, not resting, roosting, ambushing or frozen by a threat) and has under 1 BL of wrapped net displacement | VITALS-2 + STUCK-AUDIT | new |

Caveat on F (Astra I2): fullness is reserve over the body's reserve capacity, and capacity is
a search gene. Shrinking capacity raises F without more food. F stays a warning signal beside
the cohort energy trend (`cohort_energy_change`); it is not a target to optimise alone.

F and S are the vitals terms Wrysk asked for. They move before anything dies, which gives a
gradient long before L or J can. The life table adds diagnostics; those are reported and not
scored at first:
- offspring per adult that died;
- share of natives reaching maturity;
- lifespan of the dead by cause.

**Noise and feasibility** (revised after Astra's plan review,
`design/7_Research/astra-plan-review-2026-10-08.md`):
- Use 8 common seeds and identical saved starts for every arm.
- **Feasibility comes first.** An arm with a required loss on any seed (beyond the
  baseline's) is infeasible and ranks after every feasible arm. Averaging must not let one
  extinction be bought back: with a plain `mean + 0.5 sd`, an arm with P = [0 ×7, 100] beat
  a steady 35 on 7 of 8 pairs.
- Among feasible arms, rank by `mean P + 0.5 sd P` without L, and report the paired median Δ
  and the worst seed.
- "Wins k/n" is only a consistency count, not a significance test: ≥ 6 of 8 by chance is
  about 14.5 %.
- Missing required data makes a world unrankable, never P = 0. Arms pair only on the same
  objective profile and horizon. Failed runs count; they are never dropped.
- **The required roster and reference counts are declared up front** (the start library's
  founding), not read from whoever survived to the start.

**Objective parity.** Today `score.py`'s P is an external gate on census batches. The Rust
search (`cubarium-search`) still optimises its own older penalty. Until P is wired into the
search, a search's winner is only a candidate for the P gate; it is not "trained against
P".

**Slow clocks.** A 2 h screen judges access, energy, condition, stage transitions and breeding
where breeding was possible. Slow terms are marked "provisional, lifecycle untested": the
Veilhound litters once a "year" (10 h), and the lanternjaw takes about 5–7 h from egg to
adult. Finalists get staged, phase-aware 10–20 h runs.

Weights are a proposed default to publish in a training config, not tuned constants.

## 3. Run protocol

**Inner loop (one change, under 30 min).**
1. Prediction first: the budget arithmetic and a predicted number. Check t = 0 (roster,
   accessible food, stomachs, drivers, reachable land).
2. Branch 8 seeds × 2 arms from the saved d72 starts (minute 60, past founding and at dusk, so
   the dark lineages are awake).
3. Run 1.5–2 h at 1 thread, through `run-study.py --slot`, staged: check at minute 3 and a
   checkpoint every 15–30 min.
4. Early stop on a decided hard term: a required loss, or a boom over 5×.
5. The scorer prints the component table, P and the paired sign count.

**Overnight.**
- Brains and ecology are trained separately at first. A unified lock step comes only after
  each loop gives repeatable signal.
- Ecology: a narrow search, 6–12 related genes, λ 4–8, two seeds for ranking. Promote to the
  8-seed check, with the baseline in every generation.
- Brains: fixed ecology, predator-free first (v3 curriculum), σ = 0 control, and a held-out
  gate every few generations.
- Soaks: 8 seeds × one season (about 10–12 h), staged, with vitals.

**Machines: one world per physical core.** Measured 2026-10-08 on the desktop: mature d72
worlds at 1 thread, start library minute 120, binary 1179a42a.

| Worlds at once | Each runs at | Aggregate (sim-h per wall-h) |
|---|---|---|
| 8 | 3.0× real time | 24 |
| 16 | 2.2× | 35 |
| 24 | 1.1× | 26 |
| 30 | 1.0× | 31 |

- SMT siblings add contention, not throughput. `batch.py` now defaults to one world per
  physical core: 16 on the desktop, 12 on eidolon.
- An 8-seed × 2-arm × 2 h check (32 sim-hours) therefore takes about 55 min on the desktop
  alone.
- Split across the desktop and eidolon it takes about 30 min, or trim to 6 seeds or 1.5 h.
- Ticks also slow as worlds mature: about 2 ms at founding against 12–14 ms at minute 120,
  1 thread. Velvetpad keeps adding stands.
- The desktop is shared with other sessions. This morning it showed load around 120, 70 %
  iowait and 62 GB of swap in use, so check load before launching.
- EC2 is §5. Graviton has no SMT, so 57 workers on 64 vCPUs is right there.

## 4. Maps

| Map | Use |
|---|---|
| d72 (`config/desktop/terrarium-d72.toml`) | main evaluation; closest to the display |
| d96 | held-out transfer check (deeper lowland); about 1.3–1.5× the cost |
| train-flat (to build: d96 with lowland plain share 0.7, relief 0.125 m, one shelf, arch and spire) | brain foraging curriculum; rules out stuck or isolated bodies |
| patch, dry-hills, wet-valley | fast plant and grazing mechanism checks (no predators) |
| range (64 m ring) | Veilhound pack spacing and hunting |

Every map reports reachable land per lineage at t = 0, as the largest walkable component's
share, so "stuck" and "isolated" are numbers.

## 5. EC2 (plan only)

Two spot c7g.16xlarge for 12 h, independent complete jobs per host (mode A): about 128 worlds
in parallel, roughly $18–30 on spot.

Preconditions:
- the accounting and staging fixes are merged;
- the scorer reads the output;
- a 1-generation smoke passes on eidolon;
- the §6 thresholds are set;
- per-generation checkpoints, with spot interruption tested.

Launch only on Wrysk's go, and always run `down`.

## 6. Wrysk's calls

Each has a recommendation; the v3 proposal adds its own Q1–Q6. Astra's order of urgency: v3
Q1 (the brain/reflex boundary) and Q3 (what "places" a brain may sense) first; then the pack
control and the reward accounting; display targets after seeing results.

1. **Brain contract.** Staged v3 option B: sensed inputs and body actions for all nine
   lineages, reflexes reduced to body constraints, and the Veilhound pack tactic as options
   first. Recommended over A (reflexes stay over the brain) and C (brains choose among
   heuristic behaviours). Astra and the v3 author recommend B. Opus first recommended A as a
   stopgap.
2. **Until v3 lands, the live world's two brains:** keep C1, or run the two foragers on their
   heuristics.
   - Baseline v2 (§1a): C1 feeds about 2× better and moves half as much.
   - Baseline v3 repeats this on the fixed heuristics.
3. **Count the digestible stomach in the reward?** **Decided yes (Wrysk, 2026-10-09: "yeah count it").** The side-by-side is in:
   it barely changes rankings (within-world rank correlation 0.95–0.96, brain pilot part 1).
4. **Success across slow clocks:** life-cycle-aware (recommended) rather than "every 2 h run
   breeds".
5. **The weights, and what the display should look like in numbers:**
   - movement against condition: V is weighted 10 and decides C1 against heuristic today
     (§1a);
   - population band per lineage (proposed `[N0, 3N0]`, peak `5 N0`);
   - meadow patch target area (2 m² now; every baseline world has ~14 m², so it is never
     binding);
   - groves;
   - hunt frequency (proposed 0.5–2 kills per world-hour; baseline v2 has ~2).

   Best decided from captures and the baseline tables.
6. **Fidelity for brain-only training:** `water_every` 8–16, or frozen water, on train-flat?
   Recommended yes for brains only; ecology stays at 4.
7. **EC2:** one capped spot trial after the pilot. EC2-PREP readies `plan`/`run`/`fetch`/`down`
   for the new tools without launching anything.
8. **Founding birth pulse:** founders arrive at full reserve and breed at once. Bellwings went
   from 6 to 70 within 3 h and lose about 5 per seed to starvation. Leave it to the ecology
   search, or have founders earn their surplus first?
9. **Saved starts and new knobs:** Wrysk 2026-10-08: "we need to do a better job about
   changing save formats, or making them extensible, or migrations". **Done:** SAVE-EXTENSIBLE
   (aa6c4c2b) is option (b) below, and a renamed knob keeps its saved value through a per-layer
   rename table (7ff82b35). Nothing to decide.
   Background: today every new serialised knob made every saved start
   unloadable. For example, after DEN-FLOOD, loads failed with "corrupt snapshot", not even a
   clean refusal.
   - With "every number a knob", start libraries die on most merges.
   - Options:
     - (a) keep regenerating starts; about an hour of one machine each time;
     - (b) store configs in saves as self-describing tables, so a new knob takes its default
       and world-state layout changes still refuse.
   - Recommended (b). It isn't migration: a loaded world already runs new code.
   - At least, a config change should bump the schema, so the loader refuses cleanly.
10. **Decided (Wrysk, 2026-10-09): the food budget.**
    - **Keep the plant pace.** The world takes about 10 h (a year) to establish fully. Wrysk:
      "you dont really see changes in a 5 minute window watching it, but you do see changes as
      you glance at it throughout the day"; "i dont want to watch it like its a house plant".
    - **Animals must affect the plants.** Leaf-only fixes (defence, poorer food) were rejected
      because animals would still eat about 1–2 %: "the animals basically dont affect the
      plants in the world rather than being a real interconnected system".
    - **Route:**
      - raise each lineage's `mass_scale`, which scales body, reserves, gut and upkeep together
        so starvation time is unchanged (frondgrazer about ×25–75, loftstrider ×4–12,
        fruit/nectar/cap eaters ×2–6);
      - bring the gut ceiling from 18–45× upkeep to the real 3–7×;
      - keep foliage yield 0.5 (half to dung).
    - **Target:** animals eat a real share of their food (leaf eaters 10–30 % of leaf growth),
      with every lineage persisting as the hard requirement.
    - Plant defence per bite stays as a secondary search gene.
    - The extra food eaten returns as dung (litter where the herd moves) and respiration, so
      herds move nutrients ("grazing lawns").
    - Next: herding (grazers clustered for protection), raised the same day.

    Background, as first posed — **the real ecology genome** for `eco_search.py`: which levers to search (Astra: 6–12
    related genes at first). That's Wrysk's call; only a 3-gene smoke genome exists.
    FOOD-BUDGET (`design/7_Research/food-budget-2026-10-08.md`) measured the supply:
    - Animals eat 0.17 % of what plants fix. Browsers eat 0.33 % of the foliage made,
      shredders 0.03 % of the litter.
    - Cropped foliage refills in 2–4 min. Leaves live 6–29 min.
    - Gut ceilings are 18–45× upkeep; real animals manage about 3–7×.
    - A frondgrazer needs to travel 0 m/h today.

    Recommended genes, in order: plant defence per bite (×1–30; it penalises parking and leaves
    total supply alone), defence half-life, the plant clock (`@bio_rate`) or browser body mass
    (never both), senescence, fruit, graze refuge, nectar, glowcap foliage. Movement stays out
    of the objective.

    **World-model questions that come before any search** (Shape the situation):
    - plants run on a clock about 300–500× the animals';
    - a turf face holds about 4,000× natural stock density;
    - a fruit crop eaten out refills at once (by design, plants-flowering-vines 2026-10-03, so
      a picked bush feeds forever);
    - no litter decomposer role exists (Q11).
11. **Shredders by day?** The heuristic shredder is DarkActive. It sleeps while its face is
    lit, so adults lay about one egg per 2 h. Option: litter cover counts as dark, or a
    shredder on food stays awake. Visible effect: shredders feed by day and eggs roughly
    double. Litter is unchanged, since decay sets it. If you want to see litter eaten, that
    is a new decomposer role (a litter mould), not more shredders.
12. **World rules that also bind brains** (TRAPS, merged 34ef1e71):
    - on a wall face, a turn with no forward backs the body off the face it came from;
    - a body caught in solid may step along it at no greater depth (before: only out);
    - a stem climber with a carcass at its foot climbs down to it.

    - (JUVENILE-FOOD, 5a05d327) a bellwing's takeoff is paid from its reserve, then from
      its body above `body_min`, so a perched bellwing at zero reserve can still fly to food
      (before: stuck perched).
    These are motor fixes for deadlocks. Revert any you disagree with.
13. **Leads that never pay** (merged c92ce6e7, LEADS-PAY; revert if you disagree).
    - Hungry bellwings awake in the dark followed nectar scent to its peak at a bloomcrown's
      rooting face, a metre off the flower, and circled there all night.
    - Food was never short: supply by day is 2.3–5.9e-3/s against a demand of
      3.6e-4/s.
    - A heuristic forager now gives a lead up after 150 body lengths without a taste
      (`lead_give_up_bl`).
    - Over 12 min from minute 63: bellwing adult p10 fullness 0.068 → 0.111, starving adults
      1.08 → 0.46, intake +19 %.
    - Wrysk: why give up rather than fix scent lying on the ground? NECTAR-SCENT (9a5a6eba)
      fixed the world:
      - a flower's scent is now a plume in the air at its bloom, scaled by the nectar it
        holds;
      - starving juveniles 27.9 → 9.9, and give-ups fell from 293 to 6.

      The give-up rule stays as "abandon food you can't reach".
14. **Decided (Wrysk, 2026-10-09): A — stop paying for movement.**
    - Curiosity is 0 for training. The per-species knob stays in the code at 0.
    - The movement term V is a readout only: it's in no active profile
      (`config/training/fitness-v2.toml`), so a world is never ranked on movement.
    - Wrysk: "once ecology is balanced, we can revisit with a list of fine-tuning 'cosmetic'
      objective functions."

    As first posed — **Curiosity in the brain reward.** `p3-zero` (§1c) shows the curiosity term bought
    motion at the cost of food: intake −19 % midway and −32 % at the end, against −4 %
    without it.
    - It rewards moving itself, which "Shape the situation" rules out.
    - My recommendation: curiosity 0, and let scarcer or patchier food make exploration pay
      (Q10).
    - The alternative is a small share (about 0.1) that fades with age, kept only as a
      newborn's exploration prior.
    - The next brain pilot waits for the ecology genes either way: in today's world, behaviour
      barely changes intake.
15. **Decided (Wrysk, 2026-10-09): grazers herd ("C").** Wrysk: "grazers are super spread out
    today. they should be clustered more in groups for social / protection."
    - Today they group only under threat (alarm wave, bunching, hunker, calves sheltered,
      the pack's 3 m shun). Calm foragers spread out, and scanning ignores group size.
    - **Both:**
      - an innate herding pull in the heuristic, as calves follow the herd from birth; the
        heuristic is the brains' teacher;
      - the world makes herding pay:
        - many eyes: scanning shared with nearby kin, so more time to eat;
        - Veilhound packs single out the most isolated grazer;
        - predators present in training worlds.
    - Herding is never scored or rewarded.

16. **Decided (Wrysk, 2026-10-09): population targets.** Targets are set on the **wide
    world** (`config/desktop/terrarium-wide.toml`, 80 × 12 m). Wrysk: "the right balace is
    really one veilhound pack and one loftstrider family per wide d96 world".
    - **Megafauna, fixed count:**
      - one Veilhound pack and one loftstrider family per wide world, about 2–6 each;
      - a smaller world may hold them, but that isn't the balance he wants ("they maybe
        survive but dont thrive as well or kill too many other animals").
    - **The rest scale with floor area.** Per d72 area (32 × 9 m):
      - frondgrazer, littershredder, ripple-snail 8–24;
      - seedporter 4–12;
      - capgnawer 6–18;
      - lanternjaw 6–8 ("smaller to me");
      - bellwing up to about 90, swinging.

      On the wide world (×3.3): about 27–80, 13–40, 20–60, 20–27 lanternjaws, bellwing up to
      about 300.
    - Kept:
      - each species settles at 1–3× founders, with no peak above 5× (bellwings exempt);
      - hunts 0.5–2 per world-hour;
      - the hungriest tenth of adults at least 30 % full.
    - **Later, not now:** an overpopulated Veilhound lineage splits into packs that fight over
      territory.
    - The ecology search's balance check therefore runs on the wide world, about 3× a d72 run.
    - Side note from Wrysk, same day: animals move at one constant pace. They should vary
      speed, stop to pause and inspect, then move on, like real foraging or resting animals.
      This is queued as a topic.

17. **Decided (Wrysk, 2026-10-09): landscape at one year.** Targets are shares of the
    floor on the wide world at 10 h:
    - woods about a quarter, with at least two vaulttree groves big enough for a
      loftstrider family;
    - at least one big meadow (10–20 %), kept open by grazing;
    - several lanternberry groves;
    - turf and velvetpad for the rest;
    - glowcaps only in the dark.

    These replace the old never-binding targets (meadow 2 m², grove 0.5 m², one vaulttree
    recruit).
    - Soak2 finished: d72, built-in behaviour, pre-fix binary, seeds 9–16, 14 h from fresh
      founding. Medians at 14 h:
      - frondgrazer 536 (383–593) and littershredder 225, against targets of 8–24;
      - bellwing 162, ripple-snail 26, capgnawer 23, seedporter 10;
      - Veilhound 10 (the pack bred), loftstrider 4, lanternjaw 3;
      - largest wooded patch 4 tiles, vaulttrees 57 stands, glowcaps about 9,800.

      Free food lets grazers reach 20–70× the target, which supports the food decision
      (item 10). Soak2 at 9 h (d72, pre-fix) had no woods, glowcaps 31 → 10,000 and the largest
    meadow 10 m².
18. **World and behaviour direction (Wrysk, 2026-10-09), to build:**
    - **Vaulttrees taller.** Today the crown is 2.25–3.5 m, all inside the loftstrider's 3.5 m
      mouth reach. The top canopy should be out of reach even of its tendrils. Real
      reference: giraffes browse to about 6 m and elephants reach about 7 m; savanna
      acacias are 5–12 m, and saplings must grow through the browse zone.
    - **Seedporters** should get a decent share of food from latticevine fruit too.
    - **Veilhound reach.** It can rear up and jump, so lanternberry bushes (crowns ≤ 1.1 m)
      are no refuge. Only vaulttrees and mesas are.
    - **Daily rhythms:**
      - littershredder: no fixed cycle, random breaks;
      - capgnawer: night-leaning (drawn to glow, exposed to lanternjaws);
      - grazers: day, with herds staying vigilant at night;
      - Veilhound: hunts any time, and sleeps or plays a lot between hunts, like lions;
      - loftstrider: day-leaning;
      - bellwing: night-leaning, occasionally by day, huge swarms at dawn and dusk;
      - seedporter: diurnal, roosting in vaulttrees or on mesas out of predators' reach;
      - ripple-snail: as decided (night and after rain, lays on land).
    - **Movement cadence:** speed varies; animals stop to pause and inspect, then move on.

19. **Decided (Wrysk, 2026-10-09): a picked-clean crop rests (A).**
    - A lanternberry or latticevine crop that has been eaten out rests like one that dropped
      uneaten (`fruit_rest_s`, tuned by the search). Today it refills at once, about 2.3× a
      seedporter's intake.
    - Open: fruit is cheap for the plant. A crop costs about 1 % of a full shrub's income, at
      one unit of reserve per unit of fruit. Lanternberries have no flowering stage.
    - The bellwing's "pollinator" role is only a label: nothing depends on its visits.
    - **Decided (Wrysk, 2026-10-09, "yes"): flowering and pollination, and costlier fruit.**
      - Lanternberries flower before they fruit. Flowers reflect near-IR and hold a little
        nectar. Fruit sets only on flowers a bellwing visited, with a small floor for setting
        fruit without a visit, so a bellwing crash slows berries without ending them. The same
        rule may extend to latticevines and bloomcrowns.
      - The bush pays for the whole flower–fruit cycle, about 2–3 units of reserve per unit
        of edible fruit, so a crop is a real investment. The search balances the amount
        against seedporter numbers.
      - The lure mimics a fully lit flower.
20. **Decided (Wrysk, 2026-10-09): near-IR light.**
    - Bellwings are drawn to near-IR. Flowers **reflect** near-IR from the red dwarf sun, so
      they are dark at night; this is today's model and stays.
    - The lanternjaw lure **emits** near-IR, so at night it is one of the few bright
      "flowers", which is its night advantage. Wrysk: "we already know that the bellwings
      wont be attracted to the lure over the flowers" by day.
    - Glowcaps show cyan only. A bellwing mistakes one only when it is close, and only if it
      hasn't made that mistake recently (the learned expectation per class already relaxes
      back over `forget_s`).
    - An earlier reading of "glow" as flowers emitting was wrong and is withdrawn.
    - **Nectar is near-IR reflective** (Wrysk: "nectar can be made 'IR reflective'. thats
      fairly sound actually"). A flower's brightness is the light falling on it × its
      reflectance, rising with the nectar it holds. A drained flower looks dimmer, which is an
      honest signal. The lure mimics a full, fully lit flower.
24. **Decided (Wrysk, 2026-10-09): brains know places within sense range, plus remembered
    home places.**
    - The brain gets bearings to the nearest shore, climbable tree or roost it could
      perceive, and always to its den.
    - It also remembers the few key places it has used: roost trees, its water, its den.
      These fade if not revisited.
    - Escape hatches: Wrysk would "verge on potentially some high-level behavioural escape
      hatches, but idk if thats strictly necessary". Recommendation: none at the brain level
      for now.
      - The body-level ones already bind brains: TRAPS, LEADS-PAY and DROP (item 12).
      - Add one only if training shows a specific failure the world can't fix.
      - Log every trigger as a cost.
21. **Decided (Wrysk, 2026-10-09): shredders speed up decay (A).**
    - Litter near littershredders rots faster, so its nutrients return sooner to nearby
      plants (real detritivores speed decomposition by about a quarter on average).
    - Shredders also get a modest appetite gene.
    - Glowcaps stay the visible decomposer, in the dark only.
    - Microbes still do most of the work, as in real ecosystems.

22. **Decided (Wrysk, 2026-10-09): make the world cheaper first, then the two-stage
    protocol.**
    - Wrysk: "i feel like maybe we're doing something wrong and that the world shouldnt have
      to be so expensive."
    - Measured on d72, one core: today 12–14 ms per tick; plants every 2 s
      (`flora.step_every` 40) 7.4–8.9 ms; plus water every 1 s (`water_every` 20) 5.7 ms,
      2.3× faster.
    - Fauna was the rest, about 3 ms; remote senses already ran at 4 Hz.
    - FAUNA-SPEED merged, 026ea115: fauna 2.4 → 1.25 ms per tick, with census and vitals
      identical.
    - A d72 tick with the strides is now about 3.1 ms (from 12–14 ms), so a wide-world year is
      estimated at about 2.5 h, not yet measured.
    - Adopt the strides only if a side-by-side matches: `runs/stride-2026-10-09`, 8 seeds × 2 h
      from starts3r m063, old against new strides, heuristic.
      - Populations and fullness match, and P differs only 3/8.
      - Two small consistent shifts:
        - the lake is 1–7 % lower in 8/8 seeds, with more water in the air in 6/8;
        - standing foliage is about 2 % lower in 7/8, umbrellafrond and stonecushion 7/8.
      - `runs/stride2-2026-10-09` splits the cause, as median paired change against old:
        - plants every 2 s with water as before: lake −0.1 %, foliage −0.1 %, so the plant
          clock is clean;
        - water every 0.5 s: lake −1.4 %, foliage −1.1 %;
        - water every 1 s: lake −3.1 %, foliage −2.6 %.

        The shift comes from the water step and scales with it. Adopted anyway (441f553f),
        since the ledger closes and Wrysk is "not that worried".
    - **Then protocol A:**
      - screen every candidate on 8 wide worlds × 2 h from a year-old saved world;
      - year-check the best on 8 freshly founded wide worlds × 10 h;
      - judge breeding over each species' own cycle, fast breeders in the screen and
        megafauna in the year check;
      - EC2 for big rounds, on Wrysk's go.

23. **Decided (Wrysk, 2026-10-09): brain contract v3 B, with a hybrid for social
    megafauna.**
    - B for frondgrazer, littershredder, ripple-snail, capgnawer, bellwing and lanternjaw:
      the brain controls everything except physics and starts as a clone of the heuristic.
    - The B/C hybrid for seedporter, loftstrider and Veilhound: option logits plus the
      low-level actions, option 0 being own steering. Wrysk: "seedporters and loftstriders
      are social too, and even frondgrazers have rudimentary social behaviour."
    - Their menus get social skills ("stay with the troop or family"; the pack's drive,
      flank, den, rest or play). Those skills are to be written, like the grazers' herding
      pull.
    - **Architecture, a research thread Wrysk finds interesting:** "b and c seem like
      different layers of the brain … semi-independent nets with that integration layer
      between them. or literally its just a bigger net with more outputs."
      - Start with the simple form: one net with two output heads.
      - A layered form is a later study: a slow "intent" layer picking what to do about
        once a second, feeding a fast "steering" layer every tick, close to hierarchical
        reinforcement learning (options, feudal networks).

25. **Decided (Wrysk, 2026-10-09): throw away C1.** "C1 doesnt work … theres no such thing as
    'prod'". DROP-C1 (Codex) makes every lineage run its heuristic by default; v3 brains
    replace heuristics species by species.

### Build order (2026-10-09)

1. **Wave 1, running:**
   - FLOWER-FRUIT (Opus): items 19 and 20;
   - HERD (Opus): items 15 and 23;
   - SCORE-V3 (Codex): items 10, 14, 16, 17 and 22;
   - DROP-C1 (Codex): item 25.
   - FLOWER-FRUIT merged, df8dc551 (flora schema 40):
     - lanternberries: `flower_s` 900, `fruit_cost` 2.5, `pollination_floor` 0.1,
       `pollination_visits` 3;
     - vine: cost 2.5, floor 0.1;
     - `cyan_innate` 0.5 → 0.12, so one failure is enough;
     - `SenseTable::cyan_m` 1.0 m for the bellwing;
     - `nectar_nir_share` 0.5.
     - Probe (0.5 h, d72): 75 % of crops pollinated; seedporters eating fruit.
     - **Bellwings never visited a vine flower,** so vine fruit falls to the 10 % floor.
       Fix in REACH-HEIGHTS.
     - The founding bushes all flower together; staggered in DECAY-GLOWCAP.
2. **Wave 2:**
   - RHYTHMS-CADENCE (Opus): item 18, rhythms and cadence;
   - REACH-HEIGHTS (Codex): item 18, heights, reach and roosts, plus a latticevine fruit check;
   - DECAY (Codex): item 21.
   - RHYTHMS-CADENCE merged, ff5f91a0 (fauna schema 73).
     - `Activity::Leaning` with a per-lineage `rhythm` table (day / night / twilight chance,
       random rests).
     - Alarm wakes sleepers.
     - Stop-and-go pace for heuristic walkers.
     - `still_gain` 1.25 sensing after 0.5 s still.
     - The search screens read leanings (`screen::founder_sleep`).
     - Open: Veilhound pack members rest independently, where lions rest together.
   - REACH-HEIGHTS merged:
     - vaulttrees 6.5 m, so loftstriders browse 15 % of a mature crown (was 100 %);
     - `chorister.hunter.vertical_reach_m` 2.25;
     - seedporters roost above it;
     - vine flower cues sit at the face, so bellwings can sip;
     - seedporters climb to vine fruit.
     - **Open:** seedporters still barely eat vine fruit (0.0002 against 0.34 lanternberry in
       0.5 h; 14 vine visits), so vine crops stay at the pollination floor. Needs a
       diagnosis: too few visits, too little fruit, or access.
   - VINE-FRUIT diagnosis (24cf970f, census `vine:` readout only; d72 seeds 1–2, 1 h):
     - **Supply is fine:** about 4,500 vine flowers open per hour, and a hungry seedporter sees a
       bunch 90 %+ of the time.
     - **Bunches are crumbs.** 97 % set at the floor: 0.000076 organic, 1.5 % of the
       seedporter's bite half-stock. Climbs return almost nothing, and 85 % of vine fruit falls
       uneaten. Vines are 1–2 % of seedporter fruit.
     - **Per-face amounts follow face area.** A d72 face (0.125 m) gets a quarter of the
       0.25 m reference, while the eater's bite half-stock doesn't scale.
     - **Pollination is pollinator-limited.** 1 % of vine flowers are visited; bellwings drink
       only about 0.45 per hour, mostly at bloomcrowns.
     - Arms, as the vine share of seedporter fruit:
       - `spur_share` 0.01: 10–14 %;
       - plus vine floor 0.5: 37–42 %.
     - Options for Wrysk:
       - A: `spur_share` 0.01;
       - B: A plus a high floor;
       - C: bites and pollination act over a body-sized patch, not one face.
     - **Decided (Wrysk, 2026-10-09): C.** "yah do C. bellwings can also nest in the vines.
       they cant nest on bloomcrowns."
       - Seedporter bites and bellwing sips reach every vine face within reach, in metres.
       - A bellwing visit pollinates the vine flowers within its body span.
       - The small floor stays.
       - Bellwings lay their clutches on latticevine faces, never at bloomcrowns.
       - Other per-face foods (litter, film, turf, vine foliage) are audited for the same
         scale problem; fixing them is a separate decision.
       - VINE-C merged (5b69616a, fauna schema 74):
         - A bite or sip at a vine takes from every face within the mouth's reach (body
           radius plus `mouth_reach_m`). The stock is their sum, drawn in proportion.
         - A visit pollinates the open flowers within half the wingspan of where the mouth
           meets the wall.
         - `EggSite::Vine` is the bellwing's. It lays on a covered wall face within mouth
           reach, given egg headroom and open air. The young hatch clinging there.
         - With no vine, it holds its clutch: no bloomcrown fallback. A heuristic-only reflex
           flies a ready bellwing to the nearest fitting face within `flight.nest_search_m`
           (4 m, its near-IR sight range).
         - Choices to revisit: underside faces aren't nest sites; eggs stay if the vine cover
           goes; the daytime roost is unchanged (mostly ground faces and crown tops).
         - Probe (d72 seeds 1/2, 1 h, before → after):
           - vine share of seedporter fruit: 2.1/1.1 % → 5.2/5.0 %;
           - vine flowers visited: 2.7/0.8 % → 2.9/2.5 %;
           - all clutches now laid on vines;
           - bellwings alive at end: 36/48 → 30/34, with fewer eggs, and on seed 2 more were
             eaten by seedporters (4 → 10).
           - Visits stay rare because bellwings sip mostly at bloomcrowns.
         - Per-face audit (d72):
           - glassfilm E/(E+K) is 0.69–0.82 against 0.90–0.95 at 0.25 m faces;
           - litter is about 88 × the shredder's K, so it isn't limited;
           - no animal eats vine foliage (`Flora::crop` has no caller);
           - one stand per site gives stand foods 4 × the density per m² on d72.
         - The webmodel's bellwing `egg_site` fit (1.0) needs a refit.
       - **Decided (Wrysk, 2026-10-09):** "seedporters should also polinate by moving accross
         them or around them. but we can leave the numbers to ecology."
         - SEEDPORTER-POLLEN merged e298b9f0 (fauna schema 75).
           - Seedporters pollinate by contact: a flower counts when a body newly touches it
             while carrying pollen from another flower, within the climb-contact tolerance
             (`PRESSED_M`).
           - The switch is `pollen_by_contact`, on for seedporters only. Bloomcrowns aren't
             included (an open choice).
           - Probe, d72 seeds 1/2, 1 h, before → after:
             - vine flowers visited: 2.9/2.5 % → 8.8/7.9 %;
             - fruit set: 0.35/0.43 → 0.50/0.48;
             - vine share of seedporter fruit: 5.2/5.0 % → 23/20 %;
             - bellwings at end: 30/34 → 35/34;
             - no measurable cost per tick.
         - The vine's `spur_share` becomes a supply gene in the ecology genome (ECO-GENOME).
2b. **Founding (Wrysk, 2026-10-09):** "insanely slow"; "shouldnt have duplicate code";
    "short-circuit bailing for cases like 'no possible place to put this' in which case the
    terrain gets rejected, which is somethign the UI should provide feedback for rather than
    silently failing and letting you retry."
    - FOUNDING-SPEED merged with DECAY-GLOWCAP, 23fd145f:
      - founders are placed by BFS from reachable food;
      - the pre-roll runs once, and placement retries clone the settled world;
      - experiments found with `water_every=20`.
      - **d72 founds in 17–20 s (seeds 1–4) and wide in 62 s, all on the first terrain and
        first placement.** Before, both hit the 270 s timeout.
      - Water was the cost: shower 26.8 → 3.4 s and lake rest 26.1 → 2.1 s on d72.
    - DECAY-GLOWCAP, in the same merge:
      - a shredder bite speeds local litter decay (+0.25 within 0.5 m, fading over 600 s);
      - glowcaps are founded only in shade;
      - fruit crops start at mixed phases;
      - flora schema 41.
      - Probe, d72 seed 1, 0.5 h: glowcap lit share 0 → 0. Litter still rises 1.06 → 332
        organic, with shredder intake 0.165. Shredder appetite is for the eco search.
    - FOUNDING-ONE-PATH merged, c356cd11:
      - **One loop.** Every caller goes through `found::founding_loop`: the app, the census,
        the CLI and the trainers. The app stops on a terrain rejection; batch callers may
        redraw.
      - **Typed reasons** (`found::TerrainRejection`): no closed walk route, no lake, no dark
        glowcap habitat (capgnawer food), no reachable food capacity, no food perches, missing
        scenario features. Each is checked before the pre-roll.
      - **The session reports it:** `founding()` gives `state: "rejected"` plus
        `terrain_rejection` (a dictionary with a `check` key). Nothing is installed.
      - **For the UI thread:** the founding screen should read those two fields. Placing and
        styling it is theirs.
      - Callers that can't use an empty world now stop loudly. World creation, E0/E1 and
        landscapes return an error, the live trainer panics, and the census exits with
        code 3 (c5fd275a).
      - Timings, one thread: d72 seeds 1–4 in 12–13 s, wide in 44 s.
3. A wide-world start library of year-old saves.
   - SCORE-V3 merged, 97ce0994. DROP-C1 merged, bdbeb508.
   - HERD merged, 8d782d9b:
     - herding pull: frondgrazer 3 m, seedporter 4 m, loftstrider calves 6 m;
     - many eyes: scan share 0.20 → about 0.15, intake +4–7 %;
     - packs prefer stragglers;
     - frondgrazer `kin_tracks` 3 → 6.
     - The short d72 probes showed frondgrazers already 0.5–1.5 m from their nearest kin, so
       the "spread out" look is likely many tiny groups.
     - Check herd **size** on the wide world after the food genes; the pull fades past 4
       known kin (`herd_max`).
   - The wide founding probe took 472 s, including one refused-seed redraw (seed 1: bellwing
     placed 9 of 15).
   - At founding, 40 % of glowcaps stood on lit faces. Fixed by DECAY-GLOWCAP: glowcaps are
     founded only in shade.
4. The ecology search: per-lineage `mass_scale`, the gut ratio and supply genes. Screens on the
   wide world for 2 h, year checks for the best.
   - ECO-GENOME (opus-medium, brief `briefs/eco-genome.md`) is running.
     - It writes `config/training/search/eco.toml`, a budget prediction from the web model,
       and a launch-ready protocol A screen from the wide baseline's hour-10 saves.
     - It also runs a smoke check.
     - The real search starts after the baseline finishes.
   - ECO-GENOME merged b4a02be0. Its 9 genes: mass in 5 groups (Veilhound tied to grazers by a
     ratio, lanternjaw to the small eaters), one gut gene, defence and `spur_share`.
     - **Blockers it found:**
       - mass genes on a loaded save starve bodies (frondgrazers 109 → 0 in one minute);
       - this morning's saves are schema 73.
     - **Budget:** the leaf target needs frondgrazers at about ×300–900, so the box goes to
       ×25–900.
     - **Chosen (Wrysk deferred, "do whatever seems best"):** rescale bodies on load, booked as
       introduced matter.
       - ECO-RESCALE (Codex high) is doing this, with the scorer's `KeyError: 'Q'` and the
         launcher fixes.
       - Then remake the year-old start library with the current build, a new wide baseline,
         and launch the screen.
     - Stopping the old baseline was refused by the permission check, so it runs on at idle
       priority.
     - ECO-RESCALE merged 774bd98c.
       - End to end: a fresh d72 save loaded at frondgrazer mass ×40 kept 8 of 8 alive.
     - Chained detached at 11:30 (`scratchpad/wideyear2-chain.sh`, log
       `runs/wideyear2-chain.log`):
       1. build main and save the binary as `runs/bins/voxel_census.774bd98c`;
       2. run the fauna and sim tests;
       3. run the wide year baseline `runs/wideyear2-2026-10-09` (8 × 10 h, keep 11);
       4. launch `eco-wide-screen.sh` from its m000600 saves into
          `runs/eco-screen-2026-10-09`.
5. v3 brains, frondgrazer and littershredder first. The B/C hybrid for seedporter, loftstrider
   and Veilhound. Sensed places plus home memory.
   - **Bellwing nest (from VINE-C):** a bellwing now lays only on vines. Only the heuristic's
     nest reflex flies it there (`step::nest_reflex`), so the v3 bellwing brain needs three
     things:
     - its sensed and remembered places (item 24) include a vine nest site;
     - its starting clone copies the controller's final steering, nest flight included;
     - the score keeps counting energy given to young as kept (`NetEnergy::given`; it does
       today).

     A full bellwing can then bank energy only by laying, so nesting pays without a nest
     term.
   - V3-CONTRACT (opus-high, brief `briefs/v3-contract.md`) is running proposal packages P1
     then P0:
     - P1: v3 manifests and observation, H per lineage, room for the hybrid's option head,
       and the `Places` layout with a bellwing vine nest place;
     - P0: recording the executed action plus its `Steer` source.

     Still open for Wrysk before P3 and P5: the digestible stomach in the score, pack-shared
     reward (Q4b), and H 64 (Q5).
   - V3-CONTRACT merged 67a3da7a: P1 83087dd0, P0 e976056f, and a Codex fix 0240fb84.
     - Every lineage keeps its v2 slots and gets v3 blocks after them.
     - The Places hook is `observe_v3::place()`. Only the den is filled; the rest, including
       the bellwing nest, wait for P6.
     - Only the shredder and frondgrazer GRU shapes are compiled until P3.
     - Gaps:
       - the Veilhound's pup begging;
       - the lanternjaw's patrol target and its senses table;
       - the sprint label (P2).
     - The tap costs about 0.07 ms per tick.
     - Codex found that the apparent drift was water nondeterminism already on main: parallel
       proposals were summed in thread-arrival order. They're now summed in a fixed order, and
       base, tap off and tap on are identical row for row.
   - All by Codex:
     - P3 merged 71b5d8ef: a GRU per manifest (I/H/O plus an option head) for all nine, every
       output fitted by imitation, and refusals that name the wrong shape or H.
     - P2 and P4 merged 0839bc77:
       - sprint, hunker and retract are wired; lure and lift are in the response;
       - the executed sprint is recorded;
       - one `layer()`: body constraints for every body, full reflexes for heuristics;
       - the heuristic world is unchanged (238 rows identical);
       - the combined suites pass.
     - P6 merged 5ea6603c (fauna schema 76):
       - places come from the reflexes' own queries, within vision or contact;
       - they refresh at 4 Hz, staggered;
       - four remembered homes (roost, water, den, nest) fade over
         `fauna.perception.place_memory_fade_s` (600 s);
       - the heuristic world is unchanged;
       - cost with every body encoded: 1.6 → 8.1 ms per tick.
     - PLACES-SPEED merged (Codex): the added cost fell from 6.3 to 0.39 ms per tick.
       - Shore searches had taken 92.5 % of it. They now start near the body, rescan within a
         bound and reject cheaply first.
       - Exact: 13,928 place records are identical, and so are the heuristic rows.
   - P5 (reward): the digestible stomach counts (Wrysk, 2026-10-09). Still open: pack-shared
     reward (Q4b), then H 64 (Q5).
   - Ecology search redesign (Wrysk, 2026-10-09): the 12-generation CMA screen gets little done
     for its compute (84 settings in 9 genes on noisy 2 h windows; megafauna not judged). Proposed:
     map first (a space-filling design over all 9 genes, few seeds each, fitted response per
     lineage), then confirm, then the year check. Wrysk wants independent fresh-eyes plans first,
     Opus and Astra at xhigh, optimising effectiveness / total compute. The desktop CMA round is
     held (`runs/eco-screen-2026-10-09/HOLD`).
   - Fresh-eyes proposals (Opus, Astra) are in `design/7_Research/eco-tuning-fresh-eyes-2026-10-09-*.md`.
     Both say: roles triage first (Veilhound regulation, loftstrider births every 18 min, seedporter
     predation, capgnawer/shredder breeding, lanternjaw maturation, woods), then pin worlds at target
     and measure drift (thin=), fit per-lineage responses, confirm, year check. About 320 core-h
     against about 1,150 for 12 CMA generations. Awaiting Wrysk's go.
   - Plan approved (Wrysk, 2026-10-09: "i like the new plan. can do some ec2 with it to accellerate").
     Running: start-world library on main (`runs/library-2026-10-09`, 8 wide × 10 h), NEST-SPEED
     (Codex), P2/P3 probes. DRIFT-READOUT merged 35317bcb (`drift=FROM:TO`, `scripts/studies/drift.py`).
     Roles doc: `design/ecology-roles-2026-10-09.md` (3a24ff42).
   - Role call 1, loftstrider (Wrysk, 2026-10-09): A+B, a breeding season plus grown young leaving an
     over-size family, with the condition that leaving must not sink world resources: the leaver's
     matter goes to litter where it leaves. DISPERSAL (Codex) building; Veilhound dispersal built but
     off until role call 2.
   - DISPERSAL merged 1270d05c (role calls 1 A+B and 2 B1): loftstrider season, herd max 3 adults; Veilhound
     pack max 4 adults, pups don't count; leavers' matter to litter.
   - Role call 3 (Wrysk, 2026-10-09: "do A first"): vine fruit hangs ripe about 12 days, shipped 3e3098ed
     (fruit_s 1200 s, spur_share 0.58 keeps the flower cadence). Standing fruit rose 5-25x in the eidolon
     check. **Correction:** "intake unchanged" (also in 3e3098ed's message) was a misreading of
     cumulative ledger totals. Session deltas give vine intake x1.5-1.9 (shipped) and x3.8-9.7 (uncompensated).
   - SEEDPORTER-INTAKE (Opus): the root cause is the bite half-saturation K 0.005 (turf scale, inherited)
     against fruit at about 1e-4 at the mouth, so each bite is about 3 % of a mouthful. Juveniles also freeze
     in 35-54 % of samples (three heuristic bugs). Fullness 0.01 is mostly juvenile growth draw, and growth to
     adult takes about 23 h. SEEDPORTER-FIX (Codex) running: fruit-scale K, the freeze bugs, and census
     session deltas.
   - P2/P3 done (roles doc §3): grazer intake stops growing between x4 and x16 (O stays about 1-2.4 %), and x64
     takes no bites. So mass alone cannot reach 10-30 % offtake.
   - GRAZER-INTAKE (Opus) found the root cause, a units bug: the taste reference is `emit_reference x mass_scale`
     (body.rs about L2548, rule from 31e20c81), so the feed gate (cue > 0.02) rises with mass while food
     stocks don't. The gate is 0.031 at x1 and 1.96 at x64, against a max stand of 1.12. Reach is not the cap:
     about 74 % of leaf growth is within the grazer band. TASTE-UNITS (Codex) running: the reference in food
     units, with the default world unchanged. Re-derive the mass genes' boxes after it lands.
   - 2026-10-09 21:15 the session was OOM-killed (a 24 GB target copy in tmpfs /tmp); the library and probes
     resumed as user services.
   - Wade-out merged a778bd36 (Wrysk: "deffo swim out"); SMALL-FIXES (liveness ignores chosen stillness,
     `wade.swim_out` off for the insectoid lanternjaw, seedporter walk-off memory) in check.
   - Role call 4, lanternjaw (Wrysk, 2026-10-10): adult at 35 % of body max ("35%-100% adult size range ... like
     octopus"), fast mantis-like growth, rider take grows with size, riders drop off when they outgrow a small
     host, seedporters eat small juveniles, many offspring and low survival. LANTERNJAW-LIFE (Opus) building.
   - Role call 5 (Wrysk, 2026-10-10): capgnawer A, caps as its real food. Frondgrazer: "more like ungulates ...
     maybe once a day birth rate", so a birth per world day. ROLES5 (Opus) building these, plus a census
     print of nest refusals to diagnose the shredder brake.
   - Role call 6 (Wrysk, 2026-10-10: "yeah fix all four"): absolute persistence for acceptance, J counts only
     adult recruits, a bellwing seasonal envelope (median at most 3x nominal), H as prey biomass per hour.
     SCORE-FIXES (Codex) building.
   - Role call 7 (Wrysk, 2026-10-10): B, `mass_small` split per species (seedporter, bellwing, capgnawer,
     lanternjaw), with a guard keeping the lanternjaw big enough for its prey. Re-derive every mass box
     after the taste fix. Lanternjaw diet (Wrysk): "bellwings and capgnawers should be the main prey. they can
     eat shredders too. occasionally a juvenile seedporter or frondgrazer. snails can be eaten when ashore."
     Added to LANTERNJAW-LIFE.
   - Role call 8 (Wrysk, 2026-10-10): B. Veilhounds keep catching perched bellwings but rarely succeed, because
     "bellwing would be able to react very quickly". Queued after LANTERNJAW-LIFE merges (shared hunt code).
     Idea, not decided: the lanternjaw lure glows cyan and IR, "or maybe one each", to draw bellwings and
     capgnawers.
   - Wrysk (2026-10-10): grown lanternjaws fight back when attacked ("its not like attacking a grazer"), using the
     existing wound mechanics. Venom is noted for later ("lets not add too much just yet"; it would need a new
     status effect). Queued with the bellwing escape.
   - LANTERNJAW-LIFE + diet built (09cc0be1, b8329983): adult at 0.35, rider take 2x own body per day, host bears
     0.5 of its body per day, adults count 12x in hunters' size windows, clutches of 2-7 once a world day, diet
     weights. Effect check running on eidolon.
   - LANTERNJAW-LIFE merged 55f337ce. Eidolon check (4 wide x 5 h): riders outgrow shredders at about 0.094
     (rider <= 0.25 x host), 37 of 39 detachments were "outgrown", seedporters killed 29 free juveniles and
     Veilhounds 7, 2 ever adult, 0 eggs. Cause: I briefed "drop off, never switch hosts"; Wrysk (2026-10-10)
     meant "an outgrown larva looks for a bigger host, and only goes free if none is nearby" ("yes thats what
     i meant"). LJ-HOSTSWITCH dispatched.
   - Shredder breeding brake (Wrysk 2026-10-10): option A, "try option A". A shredder blocked by a crowded nest
     walks to a quieter spot to lay. The ROLES5 refusal line showed the crowded gate holding shredders about
     80 % of ready time, and one world never bred in 5 h. SHREDDER-NEST built (4ae657a6); A/B check running.
   - ROLES5's first check fed main's config to the branch arm (3 h wasted; Wrysk: "this keeps happening").
     Fixes: `scripts/studies/ab.py` (merged 4e86564b) with per-arm config, a preflight diff with `--expect`,
     and a tripwire at minute 20+ with `--expect-metric`. Then LABD, approved by Wrysk ("go with scope ...
     generic interface so that its easily extensible"): one service owns every job across desktop, eidolon
     and EC2 (EC2 only on his approve), with snapshot builds and one event feed. Dispatched.
   - LABD merged 8352f2fe and runs as `labd.service`. Agent cargo builds are now -j 8 in the idle CPU and
     IO classes (Wrysk: "my machine gets bombed with rust-lld"). LJ-HOSTSWITCH (39b9b172) is the first labd
     effect check.
   - ROLES5 merged eba6ded5 after the rerun with the branch's own config (wide, seeds 1-4, 5 h).
     - Frondgrazer: births 8.0-8.4/h against 22.8-27.2, and N 55-57 against 124-138 (predicted 40-90).
     - Capgnawer: unchanged. N was 15-42, and caps were 0-2.7 % of intake against 0-4.6 %; it still eats
       about 97 % litter.
     - CAPGNAWER-CAPS (Opus) is diagnosing why, keeping Wrysk's mechanism A.
     - HUNT-ESCAPE (Opus) is building role call 8 B (bellwings react and flush, so ground hunters usually
       miss; the lanternjaw lure must still work) and grown lanternjaws wounding attackers.
   - SHREDDER-NEST check (wide, 4 h): the walk-off frees breeding. The founders laid 57 eggs in 45 min
     (12 on main), and N reached 130-157 at 4 h, still growing about 17/h (main 21-45, target about 27-80).
     None starved, litter grew 1.3-3.4x in every world, and lanternjaws took 0-14. Held unmerged.
     - Wrysk (2026-10-10), option A: seedporters eat shredders ("sure, A works"). SP-SHREDDER (Codex) is
       building it on shredder-nest.
     - Also: "lanternjaws needa step up too though. they be slackin here. seedporters are now more
       dangerous than they are". LJ-HUNT (Opus) is diagnosing, on lj-hostswitch: is it numbers or
       per-capita hunting, and why adults don't lay? It fixes within the role and brings any role-shape
       change to him.
   - LABD-REMOTE merged 307d760e (Wrysk ran the merge and restart): ab, batch and cargo-test tasks place
     on eidolon. HUNT-ESCAPE's check is the first to run there.
   - Wrysk (2026-10-10) pointed to `design/handoffs/vault-spacing-2026-10-10.md`. SHADE-REAL (realistic
     Beer-Lambert shade for every plant, equal-height crowns shading each other, winged vaulttree seeds)
     will thin groves and shift the plant base the web is balanced on.
   - Library 2026-10-09 done: 8 seeds x 600 min, all exit 0 (built before the role changes).
   - WORLD-EDIT merged 39b02553 (Wrysk: place/remove/move plants and animals, API only): `Sim::edit`,
     session `edit {json}` plus a `pick` query, `voxel_census edit=FILE.toml` (thin, populate, place,
     remove, paint, clear). Ledger-booked, no schema bump. 1,559 crate tests and the full suite pass;
     an unedited world matches main row for row (tick_ms aside).
   - AUTOSAVE-SLOT (Codex) merged a73cbc8c: autosaves go to `<world> (autosave)` and never overwrite
     the player's save; launch opens the newer of the two (Wrysk: "like how other games do it").
     Display build display-73-autosave installed.
   - P7 (stage runs) needs P5 and P6.

## 7. Work log (2026-10-08)

| Package | Lane | State |
|---|---|---|
| Diagnoses (Opus, Astra), v3 proposal, Astra plan review | Opus, Astra (review only) | a4e627c0, 3eda1a9e, 0668cdd0, 1afd7649 |
| FOUNDING: founders arrive fed | Opus | 110cb418 |
| Staged check: sated grazers | parent | c7603d50 |
| SEARCH-DIET: 3 tests, diet coverage, deterministic seed fallback | Codex | fe99218a |
| VITALS: per-lineage vitals, plant vitals, life table, reducer | Codex | 1ff594a8 |
| ACCOUNTING: required roster, burn-in losses, exact births, stomach-aware energy (logged), R-hat, provenance | Codex | a0d78ffd |
| SIM-SPEED: d96 den-renewal pathology (5.8×), d72 −19 % | Codex | 359efe4b |
| STUDY-KIT: batch.py, score.py, fitness-v1 | Codex | dcc367c9 |
| LOAD-SET: set= on loaded starts | Opus | a5e26759 |
| TRAIN-MAPS: reach readout, train-flat(-nopred) | Opus | 84b9269b |
| STAGE-TRAIN: staged search/grid/live-train | Codex | fc41541b |
| VITALS-2: load-boundary row, segments, stuck from swallows, wrap distance, cohort energy | Opus | 1ea9dfce |
| DEN-FLOOD: walk_line_to (roof-over-den), flood depth knob | Opus | d690557b |
| SCORER-2: profiles, declared roster, feasibility gate, report | Codex | b1f05e02 |
| BRAIN-PILOT: base-chain fix, launcher | Opus | 8caf5c60 (pilot running on eidolon) |
| LOAD-CLIMB: no load bug; still climbers are Resting | Opus | 03fef559 |
| ECO-SEARCH: eco_search.py, census genome= | Opus | d6408fa2 |
| FORAGE-LEAVE: giving-up rule | Opus | 54301f40 |
| FROZEN-PREY: threat-only freeze/flee, browser wall turn, check | Opus | 0cd0767f |
| Baseline v2 (pre-fix): d72 heuristic vs C1, 8 seeds | desktop | done, §1a |
| Start library v2 (fixed heuristics): d72 seeds 1–16 | desktop | done; unloadable from schema 70 |
| Soak 2: d72 seeds 9–16 × 14 h, fresh founding, fixed heuristics (pre-TRAPS binary) | eidolon | running |
| EC2-PREP, VITALS-3 | Codex | f4d0d757, fabfc142 |
| SIM-SPEED-2: root cache, no read copy (d72 −9 %) | Opus | 1179a42a |
| STUCK-AUDIT: S as prevalence, false positives fixed | Opus | a67b64f8 |
| JUVENILE-FOOD: browsers stop to crop, push after a full turn, bellwing takeoff from body | Opus | 5a05d327 |
| Baseline v3 and the minute-120 phase check | desktop | done, §1b |
| SHREDDER-PLATEAU: stillness is gut-limited feeding; litter is decay-limited | Opus (read-only) | 972306a7 |
| TRAPS: off a face by turning, step no deeper, stem climber to carcass | Opus | 34ef1e71 |
| HUNGRY-SEARCH: legs away from covered ground for hungry heuristic foragers (schema 70) | Opus | ea20a13c |
| Brain pilot end gate: trained vs C1 vs heuristic, flat and d72, 1 h | desktop | done, §1c |
| BELLWING-HUNGER (LEADS-PAY): leads given up after lead_give_up_bl without a taste (schema 71) | Opus | c92ce6e7 |
| SAVE-EXTENSIBLE: configs saved as named-field documents over code defaults; a new knob needs no bump (§6 Q9) | Opus | aa6c4c2b |
| NECTAR-SCENT: nectar scent is a plume at the bloom, scaled by the nectar held (the real fix behind Q13) | Opus | 9a5a6eba |
| Start library v3 resaved in the new format: `runs/starts3r-2026-10-08` | desktop | chained after starts3 |
| Start library v3 and baseline v4 (TRAPS + HUNGRY-SEARCH) | desktop | chained after the end gate |
| LORE-NIR: the flowers' and the lure's light channel is near-IR, not UV (red dwarf sun); saved configs read renamed knobs through a per-layer table | Codex + parent | 7ff82b35 |
| Brain control p3-zero (curiosity 0, 20 generations) and its flat gate: curiosity caused the drift; no learning either way (§1c, Q14) | eidolon | `runs/pilot-zero-2026-10-08` |
| Wide-world year baseline after waves 1–2: wide seeds 1–8 × 10 h, heuristic, hourly saves (keep 11); populations against the wide targets, herd size, and whether year-old saves can serve as screen starts | desktop | `runs/wideyear-2026-10-09`, binary 6b75f538 (one Veilhound pack and two loftstriders per world, founded per world rather than per area), running; all 8 founded and accepted in 41–51 s |
| FOUNDING-TESTS: independent tests of the founding loop's contract; found one bug (an asked seed's post-placement rejection still returned founders), fixed | Opus + parent | 48d1482f |
| `batch.py` no longer caps memory: no address-space limit, no MemAvailable cap (Wrysk: "why are you even managing the ram") | parent | c24d6241 |
| VINE-C probe: d72 seeds 1–2 × 1 h, before and after; vine share of seedporter fruit 1–2 % → 5 %, all bellwing clutches on vines | desktop | 5b69616a |
