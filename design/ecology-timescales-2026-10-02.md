---
design_status: proposal
last_reviewed: 2026-10-02
decision_refs: []
---

# One biological clock: night, metabolism and predator cadence

A proposal by Fable, for Wrysk's decisions. This is **revision 2**, rewritten after Astra's
review (`design/7_Research/astra-ecology-timescales-review-2026-10-02.md`, verdict "revise",
12 findings). Findings are cited as [A1]…[A12]. Numbers come from
`design/7_Research/timescales-inventory-2026-10-02.md` ("the inventory") unless marked
otherwise. Nothing here is canon until Wrysk decides; every part below can be decided
separately.

## Two diagnoses, not one [A1]

### 1. The clock budget

Animal metabolism runs about 1,700–2,600× faster than nature, while the day runs only 12×
faster.
- **The metabolism figure:** the audit's browser has 1,000 s of upkeep in reserve against
  20–30 days at field metabolic rate. This compares metabolism only; plants, gestation and
  lifespans have other factors.
- **Consequence:** a 40-min night is, to an animal, about 50–80 days without food.
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
- But an animal then wakes with 10 % of its reserve, which is 50 s awake at today's upkeep.
- Sleeping prey can't dodge, so longer sleep gives predators more undefended first strikes.
  A hit does wake the prey and alert it.
- The hunter's satiated rest has its own `wake_at` (0.5), which this does not change.
- **Required tests:** a nocturnal sleeper over its 80-min daylight sleep, emergency waking,
  first-hit exposure, and post-wake food access.

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
  - **Measured phase lengths:** take each lineage's sleep windows from the clock's 0.3
    thresholds (`Activity::awake`, `fauna/src/lib.rs:567-577`). Crepuscular lineages have
    two bouts.
  - **Size of the change at today's upkeep:**
    - `h` = 0.9 alone gives 15 min at `s` = 0.5, or 25 min at `s` = 0.3.
    - With `f0` 0.8, `h` 0.9 and the 1.2 margin, passing needs upkeep about 4× lower
      (diurnal, 40 min, `s` 0.5) to about 5× lower (nocturnal, 80 min, `s` 0.3), or an
      equivalent rise in reserve per body.
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
  `(reserve − maintenance to dawn − a dawn safety floor) / time to dawn`.
  - Leaf reflush, nectar and fruit share it, in that priority order.
  - Construction respiration and material accounting stay as they are; the reflush already
    pays through `budget.build_from`.
- **The reflush threshold stays** (`reflush_below` 0.25 of foliage capacity). So this does
  **not** promise to replace the ordinary 10–13 % night leaf loss; it prevents deep losses
  from becoming a famine.
- **Nectar and fruit at night:** `night_bloom_scale` starts at 0.3, as a tunable candidate,
  and is debited from the night budget.
  - Standing stocks remain edible at night.
  - Both nectar and fruit eaters are diurnal, so this mostly fills the stock for dawn.
- **Conservation, stated precisely:** no new unbooked input. Daily fixation is **not**
  invariant, because foliage and storage room change tomorrow's capture. The initial planted
  stocks are endowments.
- **Clock modes:** time to dawn is defined for `cycle`; under `off` there is no night; under
  `local`, the date's sunrise.
- **No photosynthetic moonlight** (Astra agrees [A5]). The moon stays picture-only.
- **Glowcap** remains the light-free producer.

### 1c. Predator cadence: measure the bottleneck first [A6, A7, A8]

**Lanternjaw recruitment comes before kill cadence.** Hatching needs four things:
- footfall over a threshold;
- no adult nearby;
- the dusk draw;
- **a host within 0.75 m with a free rider slot** [A8].

The rider then drains its host until it drops off at 0.24 of adult body. No readout covers
this today.

**First step (a measurement, not a search):** add readouts for hosts encountered at dusk,
slots occupied, rider growth and drop-off, host deaths with riders, and adults established.
Then rerun the 8-world gate on the new defaults to find where lanternjaws fail.

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
  - **Arithmetic:** 2–3 adults at upkeep ÷5 cost 2.6–4.0 ou a day awake, which is
    0.67–1.0 loftstriders a day before pups. Meeting "one kill per 2 days" needs satiated
    torpor or other savings too.

**Balance as a necessary-condition ceiling only [A6].** Allocate predator demand across prey
species by energy, not by counted births. On the gate's prey births, the optimistic ceiling
is:

| ecology | lanternjaws supported at 0.56 ou a day (meal every 2 days) |
|---|---|
| C1 | about 11 |
| X21 | about 7.6 |
| defaults | about 1.25 |

- These ceilings are optimistic: newborns are smaller, other deaths take recruitment, and
  choristers compete for the same prey.
- **Use:** a generous pre-screen that refuses only the clearly impossible. Measure realized
  births, kills and consumption.
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
| **RT-D: accelerated nights** | run the night faster on the wall clock | either the sky also races (so it isn't real time), or only the ecology races, which breaks sky sync | doesn't reduce metabolic cost per night. **Not recommended.** |

**Fable's recommendation:**
- Keep `cycle` as the default.
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
- **Nectar and fruit:** same units on both sides [A5]. A sleeping seedporter needs about
  4.3 ou of fruit a day, while a stand yields about 0.55 ou a day under day plus 0.3 night
  production. That is **about 8 continuously harvested stands per seedporter**, not 4, and
  more than that awake.
- **Caps, film, litter, canopy and carnivory:** likewise.

**Targets per lineage, not one rule [A10].** Two hours can't contain a replacement
generation for:
- the lanternjaw (first breeding about 176 min, then a later dusk);
- chorister pups (2 h to independence).

Use instead:
- establishment;
- positive replenishment (reserve or biomass rising after feeding);
- feeding or kill success;
- viable offspring, or eggs and pups in escrow;

and:
- count egg-bank hatching separately from adult breeding;
- count riders separately from established hunters;
- for the chorister, require pup survival and provisioning, not a second-half litter, which
  would penalise an early success.

Keep the scorer's readiness fallback from passing a lineage with no birth.

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
