---
design_status: exploration
last_reviewed: 2026-10-05
decision_refs: []
---

# The whole-web model: the animal half

**Status:** built and partly validated; work in progress. Not canon. Plan: `README.md`.

Code: `crates/cubarium-webmodel/src/fauna/` (`config`, `laws`, `fits`, `model`, `rng`,
`tests`). Runner: `crates/cubarium-webmodel/examples/fauna_model.rs`
(`laws | young | fit [obs-only] | fitbrowser | d1b | d2b | ljdiag | answers | q2`).

## Form

- **Cohorts**: de Roos's escalator boxcar train, the individual-based limit of the
  stage-structured biomass model (de Roos et al. 2008). Each cohort is a set of identical
  bodies that steps the sim's own individual budget.
- **Stages** are read off the state: eggs and the lanternjaw's dormant bank, juveniles
  (under `birth_body`), adults, and parasite riders on a host.
- **One code path, two modes.** `Draws::Mean` is the mean field. `Draws::Twin` is the
  stochastic twin: whole bodies, and every death, refusal, hatch and lost egg is a binomial
  draw.
- **Speed.** One body over 10 h takes 10 ms. A D1b seed of three detritivores over 4 h takes
  about 13 ms at `dt` = 1 s; the whole `d1b` validation (40 runs to 4 h, 12 to 12 h) 3 s;
  `d2b` with the hunter's fit 40 s; `q2` 80 s.
  - **The cohort bound** (`max_cohorts` 48 per lineage, `min_cohort` 0.01 bodies, a pass every
    60 s). A refused clutch splits a cohort into layers and the refused, who retry after
    `nest_retry_s`; their clutch cycles never realign, so the count grew geometrically: 4,000
    shredder cohorts and 2,600 egg cohorts by m660 (profiled: `step`, the old merge and the
    arriving lifespan integral, all per cohort). Now each pass merges cohorts closer than 0.1
    in `cohort_distance` (0.5 % body, 0.5 % of the reserve cap, 3 min of age, 1 % of the
    clutch interval), folds mean-field cohorts under 0.01 bodies into their nearest
    compatible cohort, then merges the closest compatible pairs down to the bound; egg
    cohorts laid within 15 s with the same egg merge. Merges are body-weighted, so bodies and
    matter are kept exactly (tested). The 8 h observation fit went from 9 min to 3.4 s.
  - **What the bound costs**: against a bound of 96 the `d1b` cells move by at most 3 % at 48,
    8 % at 24, 19 % at 12 (the held-out juveniles at m720 the most sensitive); the old
    unbounded merge sat 5 % off.
- **Numbers.** Every rule-level number comes from `FaunaConfig`, through the scenario file's
  `base` chain and `cubarium_voxel_sim::params::Params::resolve`. None is retyped. The sim's
  pure functions are called where they exist:
  - `Gut::{throughput_factor, capacity, s_start}`;
  - `Reproduction::{clutch_for, full_clutch, brood_target, package_cost, surplus_reserve}`;
  - `SurplusWindow`, `SpeciesConfig::reserve_of`, `hunt::profitability`;
  - `Activity::awake`, `DayClock::cycle_phase` and `Sun::at`.

## Laws

All references are to `crates/cubarium-voxel-fauna/src/`. Each law is listed with what its
aggregation loses.

- **Upkeep**: `m · B · (a + (1 − a) · sleep_upkeep)`, where `a` is the awake share
  (`step.rs:878`).
  - Motor: `motor_respiration_per_s · B · effort` (`step.rs:1564`). The effort is fitted, and
    is 0 for the detritivores (they stand and feed).
  - Loses: the walk itself.
- **Intake.**
  - Bite: `bite_per_body_s · φ(B) · B · ψ`, bounded by the stomach's room (`step.rs:796`).
  - Digestion: `S/τ'`, with `τ' = τ/φ` and `φ = (B/B_max)^(e−1)`. Yields are scaled to the
    reserve's room, and the rest of the food is dung (`step.rs:822`, `2705`).
  - Mineral is assumed to fund every yield.
  - ψ is the fitted functional response.
  - Loses: meal-by-meal patchiness. The sim is bimodal (half the time sated, half searching);
    the model gives the mean.
- **Latches**: emergency at `wake_hunger`, cleared at `emergency_clear_hunger`; eating on at
  `s_start(h)·K`, off at `full_tolerance·K` (`step.rs:586-751`).
  - Hunger counts only the reserve above the brood.
- **Growth**: `growth_max · min(1, R/cap)`, taken from the free reserve above the brood
  (`step.rs:959`).
  - Never binding in the field: juveniles sit at a fullness of 0.05-0.1, so growth equals
    net intake.
- **Brood (κ-rule)**: a mature body puts `brood_share` of what is placed into the brood, up to
  `brood_target`. Respiration takes its share from the brood (`step.rs:355`, `2705`).
- **Reproduction**:
  - eligibility, hold and share through the sim's `SurplusWindow`;
  - the full-clutch wait;
  - the clutch from surplus and its refractory;
  - gestation escrow and its loss;
  - incubation, and a hatchling of `body_min` that keeps the egg's remainder as reserve
    (`step.rs:2808-3136`, `3168-3313`, `3470`).
- **Spatial terms**, both fitted:
  - **Nest refusal**: `P(Poisson(κ · crowd · (n − 1 + eggs)) > nest_crowd_max)` per attempt,
    with a retry after `nest_retry_s`.
  - **Egg site**: the capgnawer's fungus, currently 1.
- **Mortality**:
  - starvation when `B < body_min`, which takes the whole cohort;
  - the lifespan draw's survival, uniform, or the uniform × uniform arriving draw
    (`lib.rs:2210`);
  - drowning has no hazard yet: it is spatial and was never logged.
  - Loses: the spread of deaths within a cohort.
- **Hunt**: a hunter that is awake, free, and has its eating latch on kills prey bodies at
  hazard `α_prey · hunters · share²`:
  - `share` is the wound law, `profitability`. The attack chance is profitability times
    preference 1 (main's opportunistic rule), and kill per strike is the share.
  - The size window applies.
  - **The kill's reach** (fitted): that share of the corpse goes to the killer cohort's own
    carcass (`Cohort::carcass`), which only it eats, at ψ = 1, and which decays at
    `carrion_decomposition`; the rest goes to the carrion pool at once (`Returns::carrion`).
    The sim lays the whole corpse as carrion on the prey's face (`kill` and `bury_corpse`, `step.rs:2199-2245`); the
    patroller eats only what is at its mouth while it stays (`hunt.rs:45`).
  - It leaves its carcass at `1 / carcass_stay_s` (to the carrion pool); not identifiable
    from the logs, so infinite.
  - Its general carrion response is 0: the patrol eats only at its kills.
  - Each kill costs `strikes_per_kill · strike_cost_s` of awake upkeep.
  - Loses: patrol geometry, the tremor and sight senses (all inside α), and **meal
    lumpiness**: the mean field turns kills into a steady flux, so a hunter is either fed
    by it or hungry all night, where the sim's is sated a third of its night after big
    kills. The Veilhound (chorister) runs on the same code, but its α is unfitted (0).
- **Bank and riders**:
  - Bank eggs pay `egg_upkeep` and die on a log-uniform viability.
  - Dormant eggs are lost at laying at the lineage's egg-loss share, to carrion, as any
    clutch is (the sim's `clutch_fate`, `step.rs:3388`, takes dormant clutches too; the
    model had exempted them).
  - At dusk an egg hatches with chance `bank_trigger · hatch_chance_per_dusk`. The trigger is
    fitted and is currently 0.
  - A rider takes `parasite_take_per_day` of its host's body and drops off at
    `0.24 · body_max` (`step.rs:456-506`).
- **Ledger**: departures by cause, the sim's per-founder flows, births, maturations, first
  clutches, refusals, kills and strikes. Conservation is closed (tested).

## Fitted rates

Recorded in `Fits::recorded()`, with their provenance in the doc comment. The `fit` command
recomputes them.

**Responses.** These were fitted by simulation (pattern-oriented): 920 D1b windows (5 arms ×
8 seeds, minutes 20-240), model run against sim intake per window.

| lineage | response | model/sim median | IQR |
|---|---|---|---|
| shredder | litter ψ 0.43 flat | 0.97 | 0.89-1.12 |
| capgnawer | caps ψ = 0.26·C/(C + 0.93(n−1)), litter 0 | 0.78 | 0.57-1.09 |
| snail | film ψ = 0.50·F/(F + 3.4) | 1.00 | 0.73-1.39 |

- Why the inversion was dropped: inverting each window's share of the ceiling misreads
  wakefulness. The logged awake share counts bodies, while hungry juveniles wake on the latch
  and the adults sleep.
- The capgnawer's cap/litter split is not identifiable: the logs carry total intake only.

**Observation refit** (`fit obs-only`, from the 4 h fits; training seeds 4 5 7 8; count,
adults, juveniles and hatched at 360-720). Interference spans 0 to 3 × the 4 h median stock
per other body; ψ_max keeps ψ at the 4 h median stock and count, and is no longer capped at 1
(ψ itself is). A response whose interference the 4 h intake fit found (the capgnawer's) keeps
it.

| lineage | interference | aggregation κ | ψ_max | error (best / none) |
|---|---|---|---|---|
| shredder | 2.8 per other body | 2 | 0.477 | 5.57 / 6.26 |
| snail | 0.10 | 2 | 0.582 (half 3.4) | 20.27 / 20.51 |
| capgnawer | 0.93 (4 h fit) | 1 | 0.26 | flat |

- The snail's earlier 1.02 at ψ_max 1.0 sat on the grid's top and on the ψ_max cap, which cut
  ψ at the median from 0.25 to 0.19. Widened, the snail's error is flat in interference
  (20.3-21.7 up to 2 per body): its misfit is not about food sharing (below).
- The shredder's interference is weakly identified (28 per body scores 5.94).

**Behaviour.** From the D1b logs; IQRs in brackets.

- Nest crowding, `crowd/(n−1)`: shredder 0.18 [0.12-0.27], capgnawer 0.14, snail 0.47
  [0.43-0.52].
- Dark-face awake by day: 0.
- Motor effort: 0.
- Egg loss: 0 of 1,124.

**The frondgrazer** (`fitbrowser`): foliage ψ = 0.276·F/(F + 41.5), by simulation on the D1b
branch runs (no hunters), its count every hour to 720 on each seed's clamped edible foliage
(median 415), training seeds 4 5 7 8. Held out (9 12 13 15): sim 18/22/52 at 240/480/720,
model 15/27/59, all pass. D2b logs no foliage, so D2b runs use the D1b median.

**The hunter** (`fit_hunter`, the `d2` arm, minutes 120-240, 8 seeds; refitted in each `d2b`,
`ljdiag` and `q2` run, deterministic):

| rate | value | from |
|---|---|---|
| dormant egg loss | 0.333 | 24 of 72 eggs gone from the bank in the first 10 min after laying (no egg old enough to die of age) |
| general carrion response | 0 | the patrol eats only at its kills (`hunt.rs:45`) |
| kill reach | 0.358 | hunters' assimilation 5.47 summed, matched (grid 0.05-1, then bisection) |
| kill rate, shredder | 8.7e-5 per prey body per hunting LJ-s | kills matched, 125 vs 128 |
| kill rate, capgnawer | 7.6e-5 | |
| kill rate, snail | 3.9e-5 | |
| kill rate, frondgrazer | 4.4e-5 | |
| strikes per kill | 2.38 | 305 strikes / 128 kills |
| stay at a kill | ∞ | not identifiable: the mean field's kill flux tops the carcass up |

- The check is the hunting time (hungry and awake): the sim's patroller samples
  (`wait + travel + engaged`, 4 per s) give 4,513 LJ-s per seed over m120-240. The model now
  hunts 6,909 (hungry all night); before the fix 2,436 (sated half the night).
- Over the whole observation (to m600) the hunting time matches: 40,384 vs 40,109.

## Open-loop validation

The bar: the model's median falls within the sim seeds' [min..max], with one body of slack,
or within 1.5× of the median.

### D1b

Stocks are clamped per seed.

- **4 h arms** (main, gut, brood, branch, rate). Every count, maturation, first-clutch and
  clutch-mean cell passes. The arm contrasts have the sim's sign and size:
  - Maturation: rate 162 vs branch 191 vs main none by 240. The model is slower on rate (224;
    passes by factor).
  - Clutch means fall from 3 to about 1.7-2.
- **Individual at the ceiling**, model vs sim's `young_budget`: maturation 186 vs 185,
  first clutch 293 vs 290, and clutches at 372/446/546 vs 369/442/541. The capgnawer matures
  at 347 vs 345.
- **8 h observation, held-out seeds 9 12 13 15** (`d1b`, the refit as recorded): 83 of 99
  cells pass.

| lineage | N | adults | juveniles | matured | maturation | first clutch | clutch mean | hatched | refusals |
|---|---|---|---|---|---|---|---|---|---|
| shredder | 3/4 (m720 53 vs 32 [31-46]) | 2/4 (m600-720 27, 37 vs 14, 15) | 2/4 (m360-480 too many) | 2/4 | 4/4 | 4/4 | 4/4 | 3/4 | 4/4 |
| capgnawer | 4/4 | 3/4 | 3/4 | 4/4 | 3/3 | 1/2 | 4/4 | 4/4 | 4/4 |
| snail | 4/4 | 2/4 | 3/4 | 2/4 (m600 9 vs 1) | 2/2 | – | 4/4 | 4/4 | 4/4 |

- **What fails points at one mechanism**: the model's juveniles mature together, the sim's
  stall. The snail's nine juveniles of one clutch cohort all mature at m600 where one does
  in the sim; the shredder's adults reach 37 against 15. ψ is shared equally per body in the
  model; in the sim the food at a site is shared unequally (the adult at the bed, the
  juvenile off it), so a juvenile's intake under crowding is lower and more spread. Wider
  interference does not fix it (the snail's error is flat in it). It needs size-structured
  access: ψ per stage, or a within-cohort spread of ψ.

### D2b

Starts at minute 120 from the census: the prey run from founding on the median stocks, then
each lineage's adults and juveniles are scaled to the log. Hunters, riders and the bank come
from the log.

| arm | small prey at 240, sim | model | LJ kills, sim | model | result |
|---|---|---|---|---|---|
| d2 | 21 [15-28] | 22.1 | 17 | 16.8 | pass (the fit) |
| d2b-1, 1 hunter (held out) | 30 | 29.9 | 8 [4-10] | 6.5 | pass |

- **The 1-vs-3 hunter contrast reproduces**: 32 → 22 with three hunters, flat with one.
- d2b, d2b-open and d2-chosen all pass. The model has no switching, and the sim shows those
  arms close to d2.

**The lanternjaw's intake, fixed.** The model's hunter was better fed than the sim's. Its
causes (`ljdiag`, per seed):
1. **It ate every corpse whole.** The model put each kill on one carcass pool all hunters ate
   from at a full bite: 0.34 organic per kill (frondgrazers dominate), of which it swallowed
   0.16 and assimilated 0.071, against the sim's 0.043 per kill (0.68 per seed over
   m120-240 for 16 kills). Sated after every kill, it hunted only 2,436 LJ-s per seed (sim
   4,513), so the kill-rate fit doubled α to match kills: few, fast kills, every one a full
   meal. Now each kill's corpse is its killer's alone and only 0.36 of it is in reach; the
   rest is carrion.
2. **Its dormant eggs were never lost.** The sim's `clutch_fate` takes a third of them
   (flooded or the face gone) in the first ten minutes; the model had exempted dormant
   eggs. Now they are lost at the fitted share.
3. **Its prey grew unchecked.** The frondgrazer's ψ was unfitted (1): 37 vs 14 by m600, and
   the hunter's kills rose with it. Now fitted (above).
- Kill rates, sleep and digestion were checked and are not causes: awake share 0.32 in
  both, the gut is the sim's law, kills matched by construction.

| readout (d2 observation, median of 8) | sim | before | after |
|---|---|---|---|
| intake to m240 | 0.7 [0.4-1.0] | 1.2 | 0.8 |
| intake to m600 | 1.8 [0.9-2.6] | 5.2 FAIL | 2.7 pass |
| laid to m600 | 13 [11-18] | 33 FAIL | 21 FAIL (1.6×) |
| bank at m600 | 5 [2-10] | 25.7 FAIL | 10.5 pass |
| kills to m600 | 44 [17-47] | 65 FAIL | 58 pass (factor) |
| frondgrazers at m600 | 14 [2-39] | 37.7 | 21.4 pass |
| hunting LJ-s to m600 | 40,109 | – | 40,384 pass |

- **Still failing**: laying, 21 vs 13 by m600. The intake surplus left (1.5× over the
  observation) is the sim's falling kill rate per prey: 12.8 kills per hunting LJ-hour over
  m120-240, 2.1 over m240-600, three times lower per prey body. That is the patrol's
  fruitless-patch memory and prey switching, spatial, which a constant α cannot follow.
- The `main` arm (`patrol=false`, the old physiology) is not representable with d2's α. Its
  contrast with d2 is a behaviour change.

## First answers

### Q1. Generation time

Field conditions: the D1b branch arm's median stocks (litter 298, caps 12.2, film 3.6), 12
neighbours (`answers`, rerun with this branch's fits).

| lineage | maturation | to surplus (brood fill) | hold | incubation | generation |
|---|---|---|---|---|---|
| shredder | 203 | 107 | 7 | 2.6 | **320** |
| capgnawer | 596 | – | – | 3 | **none in 900 min** |
| snail | 516 | 306 | 7 | 3 | **832** |

- The shredder's 320 min matches the sim's D1b hatch → first clutch, 318 [296-375] + 2.6.
- The capgnawer (v1: 813) now lays no clutch within 900 min at these caps: with the 4 h fit's
  interference 0.93 per other body and 12 neighbours its ψ is 0.14. On D2b's caps (443) its
  generation is 515 min (Q2). The snail lays about one egg in its life.

Knobs for a 120 min generation (field):

| lineage | knob | change | result |
|---|---|---|---|
| shredder | gut throughput (`digestion_s` ÷, `bite_per_body_s` ×) | ×3.00 | 120 min, 0.73 raw/day |
| shredder | `stomach_k` and bite together | ×3.06 | 120 min |
| shredder | `yield` / `mass_exponent` / `growth_max` | any | 164 / 155 / 305 at best |
| capgnawer | gut throughput | ×8.4 | 120 min, 1.6 raw/day |
| capgnawer | `stomach_k` and bite | ×8.6 (×55 for 60 min) | 120 min |
| snail | gut throughput | ×7.6 | 120 min, 0.11 raw/day |
| snail | `stomach_k` and bite | ×7.8 (×50 for 60 min) | 120 min |
| snail | `yield` / `mass_exponent` / `growth_max` / `birth_body` | any | 410 / 374 / 821 / 795 at best |

- `birth_body` lower is blocked (`0.5 · birth_body` must hold `egg + floor`); a bigger
  hatchling is rejected by the registry's cross-rule.
- **60 min is unreachable by any single knob** for the shredder (gut floor 65 min: the hold
  7.3, incubation 2.6, `growth_max` 15 min for 0.105 of structure, day sleep).
- **Energy honesty.** Forty shredders at ×3 take about 30 raw/day against a litter flux of
  about 1,480/day. The capgnawer at ×8 would need about 1.6/day per body of caps, the snail
  at ×7.6 0.11/day per body of film: the flora half's cap and film production decide
  whether those are possible.

### Q2. Hunter carrying capacity

`q2` (80 s). Per small-prey lineage: `r` from Euler–Lotka on the one-body life history
(`history`, each clutch's age and eggs, survival the newborn lifespan draw, the egg loss);
the kill rate per prey body per LJ-day `a = α / area × share² × hunting time` (the model's
own, 2,303 s per LJ-day on d2: hungry all night); `H_max = r / a`. Then 1-6 hunters for three
world days (m120 → 480) from each D2 seed's census, the mean field and ten twins per seed (80
per row).

- **Patch**: the D2 seeds' logged stocks, median m120-600 (litter 437, caps 443, film 3.3),
  foliage the D1b median.
- **Terrarium**: `area_scale` 3.2, soak v2 stocks (litter 2,000, caps 1,700, edible foliage
  2,700), film 3.2 × patch's (soak v2 does not log film: an assumption); prey and bank 3.2 ×
  the D2 census.
- **Turnover**: as is, and the shredder's gut ×3 (its generation 311 → 117 min).

| map | turnover | r/day: shredder, capgnawer, snail | a per LJ-day: shr, cap | H_max: shredder, capgnawer |
|---|---|---|---|---|
| patch | as is | 0.44, 0.14, none (no clutch) | 0.20, 0.18 | **2.2, 0.8** |
| terrarium | as is | 0.46, 0.14, none | 0.063, 0.055 | **7.3, 2.6** |
| patch | gut ×3 | 1.49, 0.14, none | 0.20, 0.18 | **7.4, 0.8** |
| terrarium | gut ×3 | 1.53, 0.14, none | 0.063, 0.055 | **24, 2.6** |

Three days, P(a small lineage gone) in the twin, and small prey left (mean field):

| H | patch | patch, gut ×3 | terrarium | terrarium, gut ×3 |
|---|---|---|---|---|
| 1 | 0.00 (27) | 0.00 (74) | 0.00 (141) | 0.00 (255) |
| 2 | 0.06 (17) | 0.01 (64) | 0.00 (113) | 0.00 (248) |
| 3 | 0.25 (11) | 0.09 (53) | 0.00 (97) | 0.00 (240) |
| 4 | 0.41 (7) | 0.35 (42) | 0.00 (76) | 0.00 (232) |
| 5 | 0.76 (5) | 0.51 (27) | 0.00 (66) | 0.00 (218) |
| 6 | 0.88 (3) | 0.74 (17) | 0.00 (57) | 0.00 (213) |

- **The capgnawer sets H_max**: it cannot carry one hunter on patch (0.8) and carries 2.6 on
  the terrarium; a faster shredder does not change that, since the hunter takes every listed
  prey at its own rate. The lineage first lost on patch is the capgnawer.
- **The snail declines without hunters**: it lays no clutch in its life on patch's film.
  Its loss is food, not predation (D1b agrees: one snail of a dozen matures by m720).
- **Confidence**: the kill rates are fitted on 3 hunters on patch over one night; `a` on
  the terrarium assumes encounters fall with area (mass action), untested; three days is
  short against the terrarium's numbers, so its zero extinction risk says little beyond
  day 3; `r` is the mean-field individual's, which matures more readily than the sim's
  (D1b held out above), so it is an upper bound.

## What this half cannot represent

- Within-cohort spread: the sim has fast and slow individuals, bimodal satiation and unequal
  access to food by size (the held-out D1b failure).
- Meal lumpiness for the hunter (one corpse at a time).
- Space beyond the fitted rates: local litter beds, the stream, the patrol's patch memory (the
  kill rate per prey falls 3× over the D2b observation).
- Drowning of bodies.
- Patrol behaviour changes: the `main` arm.
- The bellwing's flight, and the seedporter's gliding and seed-carrying costs (motor effort
  unfitted).
- Veilhound packs (litters and provisioning are not modelled).

## The closed loop: what passes each step

Not built yet. Each step of `dt` seconds, in this order:

1. **Flora → fauna: `Supply`**, world totals in the sim's organic units, after the plants'
   own step:
   - `foliage[species]`: edible foliage per photosynthetic species (the frondgrazer's,
     loftstrider's food, summed as `Food::Foliage` and taken back pro rata); `foliage[glowcap]`
     is the cap tissue (the capgnawer's food);
   - `litter`, `film`, `nectar`, `fruit`;
   - `carrion`: the ground pool, the shredder's second food. The hunters do not eat from it.
2. **Fauna steps** (`FaunaModel::step(&supply)`), returning:
   - **`Take`**, the same fields: what was eaten, which the flora half removes from those
     stocks (foliage per species, caps from the glowcap's tissue);
   - **`Returns.litter`**: dung (`(1 − yield)` of what was digested), bite spill, and dead
     bank eggs;
   - **`Returns.carrion`**: corpses of every death but kills (the whole body, reserve and
     stomach), lost eggs, **each kill's share beyond the hunter's reach (0.64 of the corpse)
     at the kill**, carcass a hunter left, and its own carcass when it dies. The flora half
     decomposes and the shredder scavenges the carrion pool.
   - The hunter's reachable carcass stays fauna-side (`Cohort::carcass`) and decays there at
     `flora.carrion_decomposition`; that decay is respired, booked in `carcass_decayed`.
3. **Zones.** The fitted responses read world totals over `area_scale` patch-areas, so the
   loop can run on totals. Zones matter for two lineages, and the exchange would then carry
   per-zone stocks (`Supply` per zone, `Take` per zone):
   - the **capgnawer**: caps grow on dark faces and it lays only near fungus
     (`egg_site`): it needs caps per zone and the dark/fungal zones' share of its feeding;
   - the **snail**: film is the stream's; its stock per zone is its food.
   The shredder (litter everywhere, ψ flat) and the frondgrazer (foliage) can stay on totals.
   The `exchange` types are the parent's: per-zone supply is a change to ask for, not made
   here.
4. **Units and clock**: world seconds (`dt` 1 s in the fauna half), minutes in the logs; the
   flora half's checks run on their own clocks inside its step.

What the closed loop needs from the flora half, beyond stocks: the *productions* of the foods
(litter input, cap regrowth, film growth, foliage, nectar, fruit) respond to what was taken,
and the carrion pool decomposes into litter and mineral.
