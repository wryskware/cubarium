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

**Baseline (today's main, before the founding fix).** d72 and d96, shipped brains against
heuristics, seeds 1–4 × 2 h. The table follows in §1a once the batch ends.

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
| **S** | **stuck**: share of exposed bodies (awake, not voluntarily resting, no swallow for a whole 5-min window) with under 1 BL wrapped net displacement; NA without exposure | VITALS-2 | new |

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

**Machines.** Eidolon (24 threads) is the workhorse: about 16 one-thread worlds at a time. The
desktop is shared with other sessions; this morning it showed load around 120, 70 % iowait and
62 GB of swap in use. It takes only small batches until that clears. EC2 is §5.

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

Each has a recommendation; the v3 proposal adds its own Q1–Q6.

1. **Brain contract.** Staged v3 option B: sensed inputs and body actions for all nine
   lineages, reflexes reduced to body constraints, and the Veilhound pack tactic as options
   first. Recommended over A (reflexes stay over the brain) and C (brains choose among
   heuristic behaviours). Astra and the v3 author recommend B. Opus first recommended A as a
   stopgap.
2. **Until v3 lands, the live world's two brains:** keep C1, or run the two foragers on their
   heuristics. Decide from the baseline (§1a).
3. **Count the digestible stomach in the reward?** Recommended yes, after a side-by-side
   (ACCOUNTING logs both).
4. **Success across slow clocks:** life-cycle-aware (recommended) rather than "every 2 h run
   breeds".
5. **What the display should look like, in numbers:**
   - population band per lineage (proposed `[N0, 3N0]`, peak `5 N0`);
   - meadow patch target area;
   - groves;
   - hunt frequency (Astra 0.5–2 per world-hour, Opus 0.1–0.5 per pack-hour).

   Best decided from captures and the baseline table.
6. **Fidelity for brain-only training:** `water_every` 8–16, or frozen water, on train-flat?
   Recommended yes for brains only; ecology stays at 4.
7. **EC2:** one capped spot trial after the pilot.
8. **Founding birth pulse:** founders arrive at full reserve and breed at once. On seed 1–4
   checks, bellwings went from 6 to 15–42 in 10 min. Leave it to the ecology search, or have
   founders earn their surplus first?

## 7. Work log (2026-10-08)

| Package | Lane | State |
|---|---|---|
| Diagnoses (Opus, Astra), v3 proposal | Opus, Astra (review only) | committed a4e627c0, 3eda1a9e, 0668cdd0 |
| FOUNDING: founders arrive fed | Opus | merged 110cb418 |
| Staged check: sated grazers | parent | c7603d50 |
| VITALS: per-lineage vitals, plant vitals, life table, reducer | Codex | running |
| SEARCH-DIET: 3 tests, diet coverage, deterministic seed fallback | Codex | running |
| ACCOUNTING: required roster, burn-in losses, exact births, stomach-aware energy (logged), R-hat, provenance | Codex | running |
| SIM-SPEED: d96 animal-phase pathology, mature d72 profile | Codex | running |
| Baseline: d72/d96 × shipped/heuristic × seeds 1–4 | eidolon | running |
| Start library: d72 seeds 1–16, minutes 30/60/90/120 | eidolon | running |
