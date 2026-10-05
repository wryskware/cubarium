---
design_status: exploration
last_reviewed: 2026-10-05
decision_refs: []
---

# The whole-web model: the animal half

**Status:** built and partly validated; work in progress. Not canon. Plan: `README.md`.

Code: `crates/cubarium-webmodel/src/fauna/` (`config`, `laws`, `fits`, `model`, `rng`,
`tests`). Runner: `crates/cubarium-webmodel/examples/fauna_model.rs`
(`laws | young | fit [obs-only] | d1b | d2b | answers`).

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
  about 13 ms at `dt` = 1 s.
  - Known slow spot: the 8 h observation runs with nest refusals (cohort splitting and the
    O(n²) merge). The obs-only fit took 9 min. Profile before more grid fits.
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
  - The corpse goes to a fresh-kill carcass, which the hunter eats at ψ = 1 and which decays
    at `carrion_decomposition`.
  - Each kill costs `strikes_per_kill · strike_cost_s` of awake upkeep.
  - Loses: patrol geometry, and the tremor and sight senses (all inside α). The Veilhound
    (chorister) runs on the same code, but its α is unfitted (0).
- **Bank and riders**:
  - Bank eggs pay `egg_upkeep` and die on a log-uniform viability.
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

**Observation refit.** Fitted on training seeds 4 5 7 8 only; held-out validation is still
pending.

| lineage | interference | aggregation κ | ψ_max |
|---|---|---|---|
| shredder | 2.8 per other body | 2 | 0.477 |
| snail | 1.02 | 2 | 1.0 (at the grid's cap) |

**Behaviour.** From the D1b logs; IQRs in brackets.

- Nest crowding, `crowd/(n−1)`: shredder 0.18 [0.12-0.27], capgnawer 0.14, snail 0.47
  [0.43-0.52].
- Dark-face awake by day: 0.
- Motor effort: 0.
- Egg loss: 0 of 1,124.

**Kill rates.** Fitted by simulation on the D2b `d2` arm (closest to main's opportunistic rule):
kills summed over 8 seeds, minutes 120-240.

| prey | kill rate (per prey body per hunting LJ-s) | kills matched |
|---|---|---|
| shredder | 2.5e-4 | 52 |
| capgnawer | 2.2e-4 | 25 |
| snail | 1.1e-4 | 21 |
| frondgrazer | 1.8e-4 | 27 |

- Strikes per kill: 2.38 (305 strikes / 128 kills).

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
- **8 h observation** (240 to 720), before the refit:
  - Shredder N passes. Adults are too many and juveniles too few by 720 (model 29/9 vs sim
    15/23): the sim's juveniles stall under crowding.
  - Snails mature too readily (model 7 vs sim 1 matured by 720).
  - Refusals are under-predicted at κ = 1. This motivated the refit.
- The refit with interference and aggregation needs `d1b` rerun on the held-out seeds.

### D2b

Starts at minute 120 from the census: the prey run from founding on the median stocks, then
each lineage's adults and juveniles are scaled to the log. Hunters, riders and the bank come
from the log.

| arm | small prey at 240, sim | small prey at 240, model | LJ kills, sim | LJ kills, model | result |
|---|---|---|---|---|---|
| d2 | 21 [15-28] | 22 | 17 | 16.9 | pass (the fit) |
| d2b-1, 1 hunter (held out) | 30 | 29.9 | 8 [4-10] | 6.5 | pass |

- **The 1-vs-3 hunter contrast reproduces**: 32 → 22 with three hunters, flat with one.
- d2b, d2b-open and d2-chosen all pass. The model has no switching, and the sim shows those
  arms close to d2.
- **Observation to minute 600**: small prey pass. Shredders decline more slowly in the model
  (2.6 vs 0-1).
- **Fails**:
  - The lanternjaw lays too much: 33 vs 13 eggs by 600, and the bank 26 vs 5. The model's
    hunter is richer, so check `lj.intake` in the rerun.
  - The frondgrazer grows (37 vs 14): its ψ is unfitted (1).
  - The `main` arm (`patrol=false`, the old physiology) is not representable with d2's α.
    Its contrast with d2 is a behaviour change: α would need to be about 1/15.

## First answers

### Q1. Generation time

Field conditions: patch median stocks, 12 neighbours.

| lineage | maturation | to surplus (brood fill) | hold | incubation | generation |
|---|---|---|---|---|---|
| shredder | 204 | 108 | 7 | 2.6 | **322** |
| capgnawer | 501 | 301 | 7 | 3 | **813** |
| snail | 518 | 307 | 7 | 3 | **835** |

- The shredder's 322 min matches the sim's D1b hatch → first clutch, 318 [296-375] + 2.6.
- The capgnawer and the snail are food-limited: caps ~12 and film ~3.6.

Knobs for a 120 min generation (shredder, field):

| knob | change | result |
|---|---|---|
| gut throughput (`digestion_s` ÷ 3.0, `bite_per_body_s` × 3.0) | ×3.02 | 120 min |
| `stomach_k` and bite together | ×3.08 | 120 min |
| `yield` | to 1.0 | 165 at best |
| `mass_exponent` | 0.075 | 156 at best |
| `growth_max` | any | no effect: it never binds |
| `birth_body` | lower | blocked: `0.5 · birth_body` must hold `egg + floor` = 0.0675, so laying size ≥ 0.135 |
| bigger hatchling | any | rejected by the registry's cross-rule (egg + floor ≤ cap at birth_body) |

- **60 min is unreachable by any single knob.** The gut floor is 65 min, made of the hold
  (7.3), incubation (2.6), `growth_max` (15 min for 0.105 of structure) and day sleep. It
  needs gut ×≥ 5 plus `growth_max`, the hold and the sleep rule.
- The capgnawer needs gut ×7.2 for 120 min.
- **Energy honesty.**
  - The shredder at ×3 eats about 1.0 raw/day per body. Forty bodies take about 40/day
    against a litter flux of about 1,480/day (`decomposition × stock`), so litter pays easily.
  - The capgnawer at ×7 would need about 17/day of caps from a ~12-unit stock: the flora half
    must supply the cap production. The same holds for film (snail).
- Snail knob rows did not print in run v1. Rerun `answers`.

### Q2. Hunter carrying capacity

**Not computed yet.** The design:

- `r` comes from the Euler–Lotka equation on the individual's schedule (first clutch,
  interval, eggs, lifespan).
- The kill rate per prey body per LJ-day is `α × the hunting-seconds share`. The ledger's
  `hunting_s` records the hunting seconds.
- `H_max = r / a`.
- Also simulate H = 1..6 for 3 days on clamped D2b stocks.
- For the terrarium, set `area_scale` ≈ 3.2 and use soak v2 stocks: litter about 2,000,
  caps about 1,700.
- Then rerun at a 1-day turnover (gut ×3).

Measured on patch: three hunters killed about 11 small prey in one night (model 16.9 kills of
all prey, matched). Small-prey production over 2 h is ~1-3 at G ≈ 5-14 h. So three hunters
exceed balance by roughly 5×, and one roughly matches it.

## What this half cannot represent

- Within-cohort spread: the sim has fast and slow individuals and bimodal satiation.
- Space beyond the fitted rates: local litter beds, the stream.
- Drowning.
- Patrol behaviour changes: the `main` arm.
- The bellwing's flight, and the seedporter's gliding and seed-carrying costs (motor effort
  unfitted).
- Veilhound packs (litters and provisioning are not modelled).

## What the closed loop needs from the flora half

- The productions of the foods the animals eat, not just the stocks: litter input, cap
  regrowth, film growth, foliage, nectar and fruit.
- The carrion pool and its decomposition: the open loop keeps the fresh-kill carcass
  fauna-side.
- Dung and spill go to litter via `Returns`.
