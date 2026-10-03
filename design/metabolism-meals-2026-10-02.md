---
design_status: proposal
last_reviewed: 2026-10-02
decision_refs: []
---

# Realistic metabolism and meals

A proposal by Fable. Wrysk, 2026-10-02:

> food isnt filling enough for anyone … if it were they would just take less frequent bites
> … you can sleep through the night because youre still digesting and because you have
> reserves … you only need to eat a few times a day … [the gut is] not the highest priority
> … we should be sure that the expected behavior still drops out — like choristers only need
> to eat one big meal every 1-2 days.

It sits beside `design/ecology-timescales-2026-10-02.md` (revision 3, accepted in parts).
Numbers come from `design/7_Research/timescales-inventory-2026-10-02.md` unless marked
otherwise.

## The fault

Basal upkeep is 0.001 of body per second for seven lineages. Over one 120-min world day
that is **7.2 bodies**; real animals use about 2–10 % of their body per day. Three things
follow:
- The reserve (0.5 body) lasts 8 min.
- A bite is sized by current hunger, so animals nibble a little every few minutes, forever.
- There are no meals, no rests between them, and no night to sleep through.

## The model

Two changes. There is no gut.

### 1. Metabolism per world day

Each lineage's basal upkeep is set by a **daily need** `f`, the fraction of adult body
burned per world day awake at rest:
- `maintenance_per_s = f / day_length_s`;
- motor stays tied to basal by its existing ratio.

| lineage | `f` (proposed) | why |
|---|---|---|
| bellwing | 0.15 | tiny endotherm flyer: the highest mass-specific rate |
| seedporter | 0.08 | small endotherm-like glider |
| frondgrazer | 0.06 | mid-size herbivore |
| loftstrider | 0.04 | large herbivore: rate falls with size |
| chorister | 0.08 | active pack predator |
| lanternjaw | 0.04 | ambusher, low standing metabolism |
| littershredder | 0.03 | ectotherm-like |
| capgnawer | 0.03 | ectotherm-like |
| ripple-snail | 0.02 | ectotherm-like, slow |

- Today's 0.001/s is `f` = 7.2, so this cuts basal 50–360×.
- **Anchors:** about 2–10 % per day is the field-metabolic band for mammals by size (audit §2).
  Ectotherms and ambushers run lower (Secor; Geiser for torpor).
- **What it buys:** a 0.5-body reserve lasts 3–25 days of rest, so nights are trivial and
  the Part 1a night screen passes with a wide margin.

### 2. Meals: eat a meal-sized amount, then stop

Each lineage gets a **meal size** `C`, a fraction of adult body. Eating is gated by a
hysteresis on the reserve:
- **Hungry:** reserve ≤ cap − `C`. The body seeks food, and bites run at the full bite rate,
  not scaled by hunger.
- **Sated:** reserve = cap. Bites stop and the body does other things.

| term | meaning |
|---|---|
| `f` | daily need: fraction of adult body burned per world day at rest |
| `C` | meal size: fraction of adult body eaten per meal |
| `b` | bite rate: fraction of body taken per second while eating |
| meals per day | about `f / C` (more while growing or breeding) |
| one meal takes | about `C / b` |

| lineage | `C` | meals per day | target |
|---|---|---|---|
| bellwing | 0.03 | 5 | several small nectar meals |
| seedporter | 0.027 | 3 | a few fruit meals |
| frondgrazer | 0.03 | 2 | a few grazing bouts |
| loftstrider | 0.03 | 1.3 | one or two long browses |
| littershredder | 0.015 | 2 | a couple of feeds |
| capgnawer | 0.015 | 2 | a couple of feeds |
| ripple-snail | 0.01 | 2 | a couple of feeds |
| **chorister** | 0.12 | **one meal per 1.5 days** | Wrysk: one big meal every 1–2 days |
| **lanternjaw** | 0.10 | **one meal per 2.5 days** | proposal 1c: every 2+ days |

**Predators share kills.** A pack of 3 choristers eats 3 × 0.12 × 2.74 ≈ 1.0 ou of a
loftstrider's 4 ou. The rest is carrion for scavengers (the 1 h decay is unchanged). So the
next hunt comes when the reserve has fallen by `C`, after about 1.5 world days.

**One rule replaces the satiated-rest thresholds.** It supersedes the hunters' separate
`rest_at` and `wake_at` (0.9 and 0.5 today).

**Growth and breeding** draw on the reserve, as today:
- Juveniles and breeders refill more often: they are visibly the ones feeding most.
- Life histories stay fast (growth about 45 min, births every 30 min when funded). Their
  food comes from meals, not from resting cost.

**Bite rate** sets how long a meal takes, about `C / b`. The targets:
- 1–5 min for herbivores;
- under a minute for nectar;
- 1–3 min of feeding at a carcass.

Juvenile growth needs about 2.25 body per day, so `b` can't fall too low. The build sets
`b` per lineage so a growing juvenile can fund its growth while eating at most about 60 % of
its waking time.

**Sleep:**
- `wake_hunger` 0.9 and the night envelope stay.
- A body goes to sleep sated and wakes for food only if it becomes hungry.
- The `resleep_hunger` hysteresis on the hysteresis branch is folded into this rule.

**Brains and senses are untouched.** The brains still observe hunger as today, through the
sense contract; the gate decides only whether a bite happens.

## What changes elsewhere

- **Plants become relatively richer.** Animal demand falls 50–360×, so populations are
  bounded by:
  - births;
  - lifespans;
  - predation;
  - the meal gate;

  not by starvation. The search's plant genes and band target rebalance this.
- **The night endurance screen** passes trivially, so it stays as a guard.
- **The search's upkeep genes** re-base on the new `f` (×1 = this table). Intake becomes a
  bite-rate gene.
- **R/S/F** stays a readout or soft.

## Acceptance (the build's tests and measure)

1. **Meal cadence, for each lineage, in a fixture with ample food:** a sated adult's meals
   per world day are within ±30 % of `f / C`. Only the chorister and lanternjaw rows are
   checked as single meals over 1–3 world days.
2. **No nibbling:** between meals a sated adult takes no bites, even when standing on food.
3. **The night:** a diurnal adult that goes to sleep sated sleeps through the night with no
   hunger wake. A nocturnal one does the same through the day.
4. **Chorister:** a fed pack makes no further kill attempt until about `C / f` world days
   have passed, ±30 %. After a kill each adult eats about `C` and leaves the rest as carrion.
5. **Growth:** a juvenile with food in reach reaches adult size within 1.5× today's growth
   time.
6. **Measure:** the desktop terrarium on main's live defaults, seeds 1–8 × 2 h, all nine
   lineages:
   - alive at 60 and 120 min;
   - meals per day per lineage;
   - deaths by cause;
   - sleep wakes.

## Not now

- **A gut:** food-type digestion, bulky foliage against dense nectar, visible dozing after
  a big meal. Wrysk keeps it as later flavour.
- **Hunt-event cues** for watching: a separate design thread with Wrysk.
