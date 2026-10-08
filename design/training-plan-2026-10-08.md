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
| V | browser movement share below 0.30 | window meter | exists |
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
3. **Count the digestible stomach in the reward?** Recommended yes. The side-by-side is in:
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
9. **Saved starts and new knobs:** today every new serialised knob made every saved start
   unloadable. For example, after DEN-FLOOD, loads failed with "corrupt snapshot", not even a
   clean refusal.
   - With "every number a knob", start libraries die on most merges.
   - Options:
     - (a) keep regenerating starts; about an hour of one machine each time;
     - (b) store configs in saves as self-describing tables, so a new knob takes its default
       and world-state layout changes still refuse.
   - Recommended (b). It isn't migration: a loaded world already runs new code.
   - At least, a config change should bump the schema, so the loader refuses cleanly.
10. **The real ecology genome** for `eco_search.py`: which levers to search (Astra: 6–12
    related genes at first). That's Wrysk's call; only a 3-gene smoke genome exists.
11. **Shredders by day?** The heuristic shredder is DarkActive. It sleeps while its face is
    lit, so adults lay about one egg per 2 h. Option: litter cover counts as dark, or a
    shredder on food stays awake. Visible effect: shredders feed by day and eggs roughly
    double. Litter is unchanged, since decay sets it. If you want to see litter eaten, that
    is a new decomposer role (a litter mould), not more shredders.
12. **World rules that also bind brains** (TRAPS, merged 34ef1e71):
    - on a wall face, a turn with no forward backs the body off the face it came from;
    - a body caught in solid may step along it at no greater depth (before: only out);
    - a stem climber with a carcass at its foot climbs down to it.

    These are motor fixes for deadlocks. Revert any you disagree with.
13. **Leads that never pay** (HUNGRY-SEARCH): bellwings orbit a bloom or light they steer at,
    over 2,000 body lengths of path for no net way, in both arms. A give-up rule for leads
    that never reach a bite would break the orbit; it is a behaviour choice, so it waits for
    you.

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
| Brain pilot end gate: trained vs C1 vs heuristic, flat and d72, 1 h | desktop | running |
| Start library v3 and baseline v4 (TRAPS + HUNGRY-SEARCH) | desktop | chained after the end gate |
