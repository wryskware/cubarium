---
design_status: proposal
last_reviewed: 2026-10-02
decision_refs: []
---

# One biological clock: night, metabolism and predator cadence

A proposal by Fable, for Wrysk's decisions. This is **revision 3**: revision 2 was rewritten after Astra's
review (`design/7_Research/astra-ecology-timescales-review-2026-10-02.md`, verdict "revise",
12 findings), and revision 3 applies its repair-1 check ("accept with changes"). Findings are cited as [A1]…[A12] and [N1]. Numbers come from
`design/7_Research/timescales-inventory-2026-10-02.md` ("the inventory") unless marked
otherwise. Nothing here is canon until Wrysk decides; every part below can be decided
separately.

## Two diagnoses, not one [A1]

### 1. The clock budget

Animal metabolism runs about 1,700–2,600× faster than nature, while the day runs only 12×
faster.
- **The metabolism figure:** the audit's browser has a 1,000-s body-turnover time (`1/m`)
  against 20–30 days at field metabolic rate. This is the browser and reference case only;
  plants, gestation and lifespans have other factors.
- **Consequence:** a 40-min night is, to such an animal, about 50–70 days without food.
  Its full reserve lasts 500 s.
  - Seven lineages hold 0.5 body of reserve at 0.001/s upkeep, which is 8.3 min awake.
  - Hunger wakes a sleeper at 0.5, so it sleeps at most 8.3 min from full.
- **Nocturnal lineages** sleep through the light hours, about 80 min. That is twice the
  diurnal night [A1, A2]; the sleep window follows daylight's 0.3 line, not the horizon.

### 2. Food access and recruitment

This is separate, and night endurance can't fix it.
- On defaults, 7 of 9 lineages fail **before** the first night.
- Bellwing and seedporter intake is 4 % and 1 % of their outflow, while the plants grow.
- Lanternjaws barely hatch: a hatchling needs a **host** within 0.75 m, and it then drains
  that host [A8].
- Food exists but isn't reached. The two-hour targets can't see a replacement generation
  [A10].

Part 1 addresses the first diagnosis. Parts 3 and 4 address the second.

## The principle (matched to what stays) [A12]

A world day should be survivable by its animals and should set the rhythm of their lives:
- **An animal that enters its sleeping phase in a normal state outlasts that phase**, and
  can replenish in its waking phase.
- **A predator's realized intake is paced in days, not minutes.**
- **Plants smooth the night with what the day stored,** without new energy.
- **Life histories stay fast for now:** lives of 2–6 days, first breeding after about 1 h.
  Slower maturity is a later option (Part 5).

## Part 1: a consistent compressed day (recommended)

The 120-min cycle stays the ecological day for every preset and training world.

### 1a. Sleep: budget it explicitly [A2, A3]

**Rows that already exist.** Sleep depth and wake hunger are per-founder registry rows
(`fauna/src/registry.rs`). The work is to choose the values and wire the genes; no new rows
are needed [A11].

**Provisional values (tunable, not calibrated):**
- sleep 0.5 for endotherm-like lineages;
- sleep 0.3 for ectotherm-like lineages: shredder, ripple-snail, capgnawer, lanternjaw.
- The BMR torpor anchors (Geiser) don't calibrate a sim maintenance rate directly [A2].

**`wake_hunger` is a candidate, not a fix [A3].**
- Raising it from 0.5 toward 0.9 lengthens sleep.
- But an animal then wakes with 10 % of its reserve. For the seven lineages with reserve
  0.5 body and upkeep 0.001/s, that is 50 s awake.
- Sleeping prey can't dodge, so longer sleep gives predators more undefended first strikes.
  A hit does wake the prey and alert it.
- The hunter's satiated rest has its own `wake_at` (0.5), which this does not change.
- **Required tests:** emergency waking, first-hit exposure, and post-wake food access.
- **A bounded study, not a unit test:** a nocturnal sleeper over its 80-min daylight
  sleep.

**Screens, in two levels [A2].**
- **Optimistic adult screen (cheap, labelled as optimistic):** uninterrupted sleep from onset
  reserve fraction `f0` is
  `max(0, f0 − (1 − h)) × c / (m × s)`,
  where:
  - `c` is reserve per body;
  - `m` is basal upkeep per second;
  - `s` is the sleep multiplier;
  - `h` is wake hunger.

  It must be ≥ 1.2 × the lineage's measured sleeping-phase length, at `f0` = 0.8.
  - **Measured phase lengths:** take each lineage's sleep windows from its own activity
    thresholds (`Activity::awake`, `fauna/src/lib.rs:567-577`): 0.3 for diurnal and
    nocturnal lineages, 0.05/0.95 for crepuscular ones, which sleep in two bouts.
  - **Acceptance is computed exactly, not rounded.**
  - **Check `f0` is reached:** verify that animals actually enter each sleep at
    `f0` ≥ 0.8, cycle after cycle. Balancing total daily intake is not enough.
  - **Size of the change at today's upkeep:**
    - From a full reserve (`f0` = 1), `h` = 0.9 alone gives 15 min at `s` = 0.5, or 25 min at
      `s` = 0.3.
    - At `f0` 0.8, usable reserve is 0.35 body: 700 s of sleep at `s` 0.5 and 1,167 s at
      `s` 0.3.
    - With the 1.2 margin, passing needs upkeep 4.11× lower (diurnal, 40 min, `s` 0.5) to
      4.94× lower (nocturnal, 80 min, `s` 0.3), or an equivalent rise in reserve per
      body.
- **Whole-cycle check:** over one day, the waking phase's feasible intake must cover the
  day's costs, adding to basal:
  - motor;
  - juvenile growth;
  - gestation;
  - healing;
  - rider drain (a lanternjaw hatchling costs its host +5 % of awake basal, about 14 % more
    endurance loss at `s` = 0.3) [A8];
  - wake interruptions (alert, den travel).

  This uses measured activity, so it's a readout first and a screen once validated.

**Founders arrive with a full reserve,** which is a preset knob. This is a founding fix and
says nothing about sustainability [A1].

### 1b. Plants: spend the store at night, with no unbooked input [A4]

- **Mechanism:** one spendable night budget per stand:
  `max(0, reserve − maintenance to dawn − a dawn safety floor) / max(time to dawn, a floor)`.
  - Leaf reflush, nectar and fruit share it, in that priority order.
  - Each keeps its existing rate and stock caps.
  - Construction respiration is charged against the same budget.
  - Material accounting is unchanged; the reflush already pays through
    `budget.build_from`.
- **The reflush threshold stays** (`reflush_below` 0.25 of foliage capacity). So this does
  **not** promise to replace the ordinary 10–13 % night leaf loss. It lowers famine risk
  after deep losses; it cannot guarantee against famine when stores or reachable foliage
  run short.
- **Nectar and fruit at night:** `night_bloom_scale` starts at 0.3, as a tunable candidate,
  and is debited from the night budget.
  - Standing stocks remain edible at night.
  - Both nectar and fruit eaters are diurnal, so this mostly fills the stock for dawn.
- **Conservation, stated precisely:** no new unbooked input. Daily fixation is **not**
  invariant, because foliage and storage room change tomorrow's capture. The initial planted
  stocks are endowments.
- **Clock modes:** "dawn" is always the **ecological** dawn.
  - Under `cycle`, the cycle's dawn.
  - Under `off`, there is no night.
  - Under today's coupled `local` mode, the date's sunrise.
  - Under RT-A (Part 2), the 120-min ecological clock's dawn, never the wall clock's.
- **No photosynthetic moonlight** (Astra agrees [A5]). The moon stays picture-only.
- **Glowcap** remains the light-free producer.

### 1c. Predator cadence: measure the bottleneck first [A6, A7, A8]

**Lanternjaw recruitment comes before kill cadence.** Hatching needs four things:
- footfall over a threshold;
- no adult nearby;
- the dusk draw;
- **a host within 0.75 m with a free rider slot** [A8].

The rider then drains its host until it drops off at 0.24 of adult body, or until the host
dies and frees it. Lower prey upkeep also lowers a rider's income, so rider growth must be
checked whenever prey upkeep moves.

**Readouts that exist:** `voxel_census` already exports `riders_now`, `riders_attached`,
`rider_dropoffs` and `riders_freed` (`crates/cubarium/examples/voxel_census.rs:986,1184-1187`)
[N1]. The gap is attributing the bottleneck.

**First step (a measurement, not a search):**
- Add readouts for hosts within reach at dusk, rider growth, and adults established.
- Rerun the 8-world gate on the new defaults to find where lanternjaws fail.

**Targets, defined as realized intake (not kill counts):**
- **Lanternjaw (nocturnal ambusher):** a meal of about one grazer-equivalent every 2+ days.
  Satiated torpor already exists. Real ambushers eat every 10–40 days and active small
  predators daily (Secor), so this is compressed but in order.
- **Chorister pack:** about one large kill per pack per 1–2 days.
  - The pack code **already admits large prey** by the summed body mass of the nearby adults
    (`pack.rs:79-97`). Two full adults take an 8.2 ou cap, enough for the 5.28 loftstrider.
  - The proposed pack prey-cap lever is withdrawn [A7].
  - The levers that remain are chorister upkeep and the number of founding adults.
  - Pups add 1.1–2.1 ou per litter at birth, plus 2 h of growth and upkeep, delivered only at
    the den above 60 % giver reserve.
  - The first litter is not delayed by the 6 h interval.
  - **Arithmetic:** 2–3 adults at upkeep ÷5 cost:
    - **today's rhythm** (diurnal, sleep 0.5 at night): 2.64–3.96 ou a day, which is
      0.67–1.0 loftstriders a day;
    - **awake all day:** 3.16–4.74 ou a day, which is 0.80–1.20 loftstriders a day.

    Both are before pups, provisioning at 0.7 efficiency, and the nearby-carcass feeding
    exception. Meeting "one kill per 2 days" needs satiated torpor or other savings too.
  - The founding pack size doesn't cap later adults.
  - **Rhythm:** the chorister stays diurnal. A crepuscular chorister is not proposed now; if
    tried, evaluate it across a whole litter cycle first.

**Balance: an illustration, not a screen [A6].** Allocate predator demand across prey
species by energy, not by counted births. On the 2026-10-02 gate arms' prey births per
world-day, the conditional ceiling is:

`N = 0.5 × (1.125 × grazer births + 0.28125 × shredder births) / 0.5625`

The assumptions behind it:
- half of the prey energy goes to lanternjaws;
- every newborn prey is counted as an eventual full adult;
- each lanternjaw takes half a grazer-equivalent a day.

| gate arm | lanternjaws supported |
|---|---|
| C1 (C1's ecology and brains) | 11.0 |
| X21 (C2's ecology, C1's brains) | 7.6 |
| defaults | 1.25 |

- These ceilings are optimistic: newborns are smaller, other deaths take recruitment, and
  choristers compete for the same prey.
- **Use:** an illustration only. Historical birth rates and the 50 % share are not a
  hard rejection for a new full-roster candidate. Measure realized births, kills,
  non-predatory deaths, maturation, and competing predator and rider demand.
- The claim that this "is why the target was unreachable" is withdrawn [A6].

### 1d. Unchanged

- Life histories.
- The 2 h training horizon.
- The brains' architecture.

## Part 2: real time (Wrysk's choice; needs a clock split) [A9]

`local` mode gives a 12 h night, varying with season when latitude and longitude are set. The
options:

| option | what it means | what you see | work and risk |
|---|---|---|---|
| **RT-A: sky only** | **two clocks**: the picture follows the wall clock; photosynthesis, sleep, hatching and climate read the 120-min ecological clock | a real-time sky over a world with its own short days: night hunters can be out under a daytime picture | a clock-split package; today's `local` mode doesn't do this |
| **RT-B: slow world** | divide every biological rate by 12 and multiply every biological duration by 12 (lifespan, gestation, egg viability, pack intervals); speeds stay real | lives of **2–6 real days**, a few births and hunts a day | searchable headless (cost scales with ticks, not wall days), but much more expensive per candidate. Real speeds change encounters and transport, so retune |
| **RT-C: moonlight + torpor** | plants earn 5–10 % of daylight at night; animals torpor | everything sleeps at night, so the nocturnal niche disappears | 5–10 % is about 19,000–38,000× real moonlight. Torpor at 5–10 % still leaves small lineages only 1.4–2.8 h at today's upkeep. **Not recommended.** |
| **RT-D: accelerated nights** | three different things: (1) the biology and its sky race together at night; (2) only the ecological sky advances faster while biology runs normally; (3) the biology races under an unchanged real-time night | (1) is real time only by day; (2) the ecological night is short but out of sync with the real sky; (3) the real night stays | (1) keeps the cost per ecological night; (2) **reduces** it, at the price of sky sync, which makes it effectively RT-A; (3) **increases** it. Only (2) helps, so it folds into RT-A |

**Fable's recommendation:**
- Keep `cycle` as the default.
- Every night-budget calculation uses the ecological clock.
- If a wall-clock sky is wanted, RT-A, built as an explicit clock split.
- RT-B only as a soak experiment.

## Part 3: before the full-roster search [A10, A11]

**Exhibit a passing candidate first.** Before CMA starts, show one candidate that passes
every analytic screen. Then list the actual per-lineage gene ranges and ties, including:
- reserve per body;
- sleep and wake;
- intake factors.

**Gene ranges must reach the targets.** Registry upkeep bounds are 2.5e-4–4e-3/s, so the
lanternjaw and chorister reductions need wider rows. The upkeep tie also lowers bite rate
and motor, so an intake gene must be able to compensate.

**R/S/F per food type.** It reads frondgrazer and foliage only today. Each consumer needs an
access check against its own food:
- **Nectar and fruit:** same units on both sides [A5].
  - A sleeping seedporter needs about 4.28 ou of fruit a day, while a stand yields about
    0.554 ou a day under day plus 0.3 night production.
  - That is **about 7.7 continuously harvested stands per seedporter**, or 9.3 if it never
    sleeps.
  - These are optimistic lower bounds: default adult basal costs, continuous harvest,
    enough plant reserve.
  - The candidate check must use actual yield, reach, caps, fruit drops, motion and
    reproduction.
- **Caps, film, litter, canopy and carnivory:** likewise.

**Targets per lineage and life stage, not one rule [A10].** Two hours can't contain a
replacement generation for:
- the lanternjaw (first breeding about 176 min, then a later dusk);
- chorister pups (2 h to independence).

Use instead:
- establishment;
- positive replenishment (reserve or biomass rising after feeding);
- feeding or kill success;
- viable offspring, or eggs and pups in escrow;

How to apply them:
- **Require actual births where a birth fits in the horizon.** Elsewhere, use explicit
  progress criteria.
- **Readiness never substitutes for a required birth.**
- **Replenishment must cover a whole cycle's measured spending.** A rise after one meal can
  hide a negative day.
- Count egg-bank hatching separately from adult breeding.
- Count riders separately from established hunters.
- For the chorister, require pup survival and provisioning, not a second-half litter, which
  would penalise an early success.

**Horizon and finalists.** Keep 8 worlds × 2 h with early stopping for candidates. Validate
**finalists** from saved worlds across the next dusk, pup independence and a litter
interval. WORKING_POLICY allows longer checks for slow-clock reasons like these.

**Brains.** Hold C1's two brains fixed as a controlled experiment, but **remeasure** their
movement and feeding under the changed costs and rhythms. The lock step follows.

## Part 4: the order of work

1. Part 1a and 1b mechanics and screens, with their tests:
   - wake and sleep wiring;
   - the night budget for plants;
   - founders full.
2. Lanternjaw recruitment readouts (1c), then a diagnostic gate on the new defaults.
3. Part 3 groundwork:
   - genes and ranges for all nine lineages;
   - per-food access screens;
   - per-lineage targets;
   - an exhibited passing candidate.
4. The full-roster ecology search, overnight, with brains fixed.
5. Finalist validation on longer clocks.
6. The lock step.

Astra reviews the screens and targets (3) before the first overnight run.

## Part 5: later, not now

Slower maturity ("several days to adulthood") needs training horizons of 3+ days, or
finalist-only validation. Decide after Part 4.

## Decisions for Wrysk

1. Part 1: sleep budgeting with `wake_hunger` as a tested candidate, the plants' night budget
   with night bloom, and full founders. **Recommended.**
2. Real time: RT-A as a clock split (recommended, if a wall-clock sky is wanted), RT-B as an
   experiment, or neither for now.
3. Predator targets as realized intake: a lanternjaw meal every 2+ days, a pack kill every
   1–2 days.
4. The order in Part 4, including the lanternjaw recruitment diagnosis before any predator
   tuning.
