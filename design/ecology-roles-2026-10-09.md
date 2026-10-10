---
design_status: exploration
last_reviewed: 2026-10-09
---

# Ecology roles triage, 2026-10-09 (stage 0 of the approved tuning plan)

Proposals for Wrysk, one decision at a time. **Nothing here is decided.** Section 1 is the
evidence: what limits each animal in the year-old baseline. Section 2 has the items that
numbers can't fix, highest leverage first, each with 2–3 options and a recommendation.
Section 3 covers the probes (P1 done; P2 and P3 running).

Tags: **[checked]** means read from the code or the run data; **[hyp]** means a hypothesis
or estimate. Code names are in brackets.

**Data.** `runs/wideyear2-2026-10-09/heuristic/s1..s8`: 8 wide worlds × 10 h (one year) at the
shipped numbers, binary `774bd98c`. All 8 seeds finished. Life records are deduplicated by
body id. "Wide" is 3.3× the d72 floor, so the wide bands are:
- frondgrazer, littershredder, ripple-snail 26–79; seedporter 13–40; capgnawer 20–59;
- lanternjaw 20–26;
- Veilhound and loftstrider 2–6 each (one pack, one family);
- bellwing: no band, only a ceiling of 2,970 (Wrysk's own figure is about 300).

Code citations are to `main`. The life-history code and `config/desktop/terrarium-wide.toml`
read the same at `774bd98c`; in `step.rs` the line numbers sit about 190 lines lower there.

## 1. Demography: what limits each animal

Rates are per animal per sim-hour over all 8 seeds. Growth r is ln(N600/N360)/4 h, mean ± sd
over seeds. Fullness p10/p50/p90, "intake ÷ upkeep" (intake over upkeep plus motor, which
includes the matter going into growth), and "eligible" (the share of bodies able to breed)
are means over minutes 360–600. [checked: `scratchpad/eco-roles/demog.py`; vitals and life
records]

| Animal | N at 10 h (8 seeds) | r per h | births | deaths (cause) | fullness | intake ÷ upkeep | eligible |
|---|---|---|---|---|---|---|---|
| frondgrazer | 208–448 | +0.20 ± 0.03 | 0.23 | 0.027 (Veilhound 0.023, age 0.003) | 0.05/0.18/0.51 | 29× | 10 % |
| loftstrider | 14–101 | +0.22 ± 0.08 | 0.27 | 0.022 (all Veilhound) | 0.05/0.16/0.51 | 39× | 14 % |
| Veilhound | 8 every seed | 0 after the litter | 1 litter of 4 | none in 80 world-hours | 0.12/0.47/0.65 | 3.8× | 62 % |
| seedporter | 3–22 | −0.15 ± 0.10 | 0.013 late | 0.134 late (Veilhound 0.114) | 0.00/0.01/0.13 | 3.5× | 1 % |
| capgnawer | 3–58 | −0.17 ± 0.17 | 0.066 | 0.125 (age 0.106, lanternjaw 0.014) | 0.49/0.61/0.94 | 25× | 39 % |
| littershredder | 8–59 | −0.05 ± 0.10 | 0.119 | 0.127 (age 0.102, lanternjaw 0.026) | 0.19/0.72/1.00 | 23× | 65 % |
| lanternjaw | 6–16 | −0.06 ± 0.08 | dusk hatches only | 0.067 (all Veilhound) | 0.02/0.08/0.22 | 8.9× | 0 % |
| bellwing | 211–872 | +0.06 ± 0.08 | 0.42 | 0.37 (starved 0.27, Veilhound 0.06) | 0.04/0.12/0.26 | 3.1× | 17 % |
| ripple-snail | 63–96 | −0.03 ± 0.02 | 0.041 | 0.070 (age 0.041, lanternjaw 0.028) | 0.00/0.16/0.55 | 12× | 9 % |

"Late" means minutes 360–600.

**Cohorts** (native-born bodies; still alive at 10 h means the age is censored) [checked]:
- **frondgrazer:** 2,827 born; 908 reached adult, median 3.8 h.
- **loftstrider:** 342 born; 124 reached adult, median 3.2 h.
- **seedporter:** 168 born, 156 of them in hour 0, from the founders' starting reserves.
  - None reached adult. 104 died young; 64 are alive juveniles aged about 9.9 h.
  - Births after hour 0 run at about one per hour across all 8 worlds.
- **capgnawer:** 137 born, 69 of them in hour 0; 63 reached adult, median 5.9 h.
  - Eggs were laid in only 4 of 8 seeds. s4, s5, s7 and s8 laid none in 10 h.
- **littershredder:** 200 born; 104 reached adult, median 3.1 h.
- **lanternjaw:** 112 hatched, every one a dusk hatch from the founding egg bank.
  - 10 reached adult, median 7.7 h, all in s3, s5 and s6. One egg was laid in 80 world-hours.
  - 30 of the 112 were killed by the Veilhound, 29 of them juveniles. The other 82 are alive.
  - Riders on littershredders mostly never reach drop-off: 24 are still riding at 9 h.
  - Juveniles that have dropped off grow at a median 0.025 body units per hour.
- **Veilhound:** one litter of 4 per seed at about minute 217. Pups become independent at about
  minute 577.

**Kills** [checked, life records]:
- **Veilhound, 1,539 kills:** bellwing 1,039 (68 %), frondgrazer 285 (236 of them juveniles),
  seedporter 151, loftstrider 34, lanternjaw 30.
- **Seedporter:** 1,634 bellwings. That carrion is 46 % of the seedporter's intake; fruit is
  54 %.
- **Lanternjaw:** bellwing 199, ripple-snail 131, littershredder 24, capgnawer 10.

**Diets** [checked, intake by food in the life records]:
- The capgnawer eats 98 % litter and 2 % caps.
- The littershredder eats 99 % litter.

**Limiting factor, per animal:**
- **Frondgrazer: nothing.** Food is unlimited (intake 29× upkeep, leaf offtake 0.5–1.3 %).
  Predation takes 0.02/h against 0.23/h born. This is the numbers route (mass and gut), and
  P2 tests it. It isn't a role item.
- **Loftstrider: life history.** It breeds every 18 min, its calves are adult in 3 h, and
  food never binds. → §2.1
- **Veilhound: life history.** One litter of 4 in year 1, and nothing removes hounds in later
  years. → §2.2
- **Seedporter: access to fruit, then predation.** About 96 % of vine fruit falls uneaten.
  Seedporters are hungry 98 % of the time and hardly breed. Hungry animals on the ground are
  the Veilhound's prey. → §2.3
- **Lanternjaw: maturation (life history plus juvenile food).** At 0.025/h, a juvenile takes
  about 15 h to reach adult size; the code's own rule caps maturation at 6 h. Veilhounds eat
  the dropped juveniles. → §2.4
- **Capgnawer: breeding access.** It eats litter rather than caps, so it rarely stands where
  its eggs may go (beside capped glowcaps). Four seeds laid nothing. → §2.5
- **Littershredder: breeding after eligibility.** 65 % are eligible but births run 10–20×
  below what the timers allow [hyp: the crowded-nest brake]. → §2.5
- **Bellwing: food (starvation).** It regulates itself; nothing to change. The scoring
  ceiling question is §2.8.
- **Ripple-snail: food (film) and old age.** It declines slowly at −0.03/h. Leave it to the
  `mass_snail` number.

**Stocks growing without check, and the role that should answer each** (the "balance
through roles" rule):
- **Frondgrazers** (0.23/h born, 0.02/h dying): the Veilhound should answer, but 68 % of its
  kills are bellwings.
- **Loftstriders:** no answer at all.
- **Litter** (about 3 → 16,000–24,000 units per world): the littershredder is breeding-limited,
  and the capgnawer eats litter as well.
- **Glowcap stands** (3–6 → 8–104): the capgnawer should answer but eats 2 % caps.
- **Fallen fruit** (about 70 units a year per world): no answer.

## 2. Role calls, in order

### 2.1 Loftstrider life history

**Now** [checked]:
- Every lineage shares one `birth_interval_s` of 1,098 s. The loftstrider's gestation is
  329 s (`terrarium-wide.toml:115-120`).
- A newborn is adult in about 3.2 h, and the median lifespan is 16 world days, 32 h
  (`body.rs:142-150`).
- Per animal: births 0.27/h, deaths 0.022/h (all Veilhound), so N doubles every 3–4 h
  (2 → 14–101 in a year).
- Food never binds: intake is 39× upkeep and total leaf offtake is about 1 %.
- At ×12 mass and 6 animals, demand would be about 3× today's loftstrider intake, still a
  small share of leaf growth [hyp]. So the `mass_loftstrider` box (×4–12) can't regulate it
  except by starving it.

- **A. A breeding season (no code).**
  - Mechanism: one calf per adult per year, in a window, with a longer gestation. This is
    the Veilhound's season gate [`litters_per_year`, `breeding_phase`, `breeding_window`],
    which the gestation path applies to any live-bearer (`step.rs:4242`, gate at
    `lib.rs:2052`).
  - Budget: births are capped at about 0.8 per adult per year (most calves are adult by the
    next window). Deaths are about 0.5 per animal per year (Veilhound 0.22 plus old age 0.31).
    So r is about +0.3 per year instead of +2.2 per year.
  - Prediction: 2 → 3–4 in year 1, passing 6 in about year 3–4 [hyp].
  - Cost: config only (3–4 lines in `terrarium-wide.toml`).
- **B. A plus young leaving the family.**
  - Mechanism: when a family is over its size, a maturing young animal leaves the world
    (emigration, booked as removed matter).
  - Budget: N stays between the adult count and adults plus one year's calves, so 2–6 with a
    family of 2–3 adults.
  - Cost: one emigration mechanism, about 0.5 day of code. It is shared with the Veilhound
    (§2.2 B).
- **C. Let food regulate it.**
  - Mechanism: rely on the mass box and gut throughput.
  - Budget: needs mass far beyond ×12 before food binds at 6 animals [hyp]. It would starve
    calves rather than slow births.

**Recommendation: A now (config only), with B as the multi-year regulator, built once for
both large animals.**

### 2.2 Veilhound pack regulation

**Now** [checked]:
- Configuration (`terrarium-wide.toml:189-205`):
  - one litter a year, conceived in a window at phase 0.2–0.35 (about minutes 120–210);
  - gestation 6,000 s at 1.5× upkeep;
  - pups fed at the den until 21,600 s (6 h).
- Litter rule [`Pack::condition_litter`], `body.rs:930-935`, applied in `litter_size`
  (`step.rs:4489`):
  - a breeder at reserve fraction r has `1 + floor(c × 4)` pups, capped at 4;
  - c = (r − 0.6)/0.4;
  - conception needs r ≥ 0.6 (`body.rs:984`).
- Every seed had 4 pups (the breeder was near full), and no Veilhound died. Pup starvation
  exists [`pup_starves_on_empty`, `body.rs:988`] but never fires, because the pack is well fed
  (intake 3.8× upkeep, 68 % of kills bellwings).
- Lifespan is 12 world days, 24 h (2.4 years).
- Budget with no other change: 4 pups a year against about N/2.4 deaths a year gives
  N* ≈ 9–10.

- **A. Litter by condition only (as now).**
  - Mechanism: the litter tracks the breeder's reserve.
  - Budget: needs the breeder below r 0.8 at conception for 2 or fewer pups. With bellwings
    as easy food it stays near full [hyp].
  - Prediction: 8 every year 1, and N* about 9–10 over the years.
  - Cost: none.
- **B. Young leave the pack (subadult dispersal).**
  - Mechanism: at independence or maturity, a young hound over the pack's size leaves the
    world, the way young wolves leave at 1–3 years.
  - Budget: N swings between the adult count and adults plus the litter.
    - With 4 adults and 4 pups: 4–8, averaging about 6.4 if pups count.
    - With litters of 2 or fewer: 4–6.
  - Cost: the shared emigration mechanism (about 0.5 day).
- **C. The litter follows prey per hound.**
  - Mechanism: the litter reads the prey biomass in the territory per hound rather than the
    breeder's reserve. The world's food then sets the litter (a situation, not a script).
  - Budget [hyp]: at target prey numbers (26–79 frondgrazers), litters drop to 1–2.
  - Cost: about 0.5–1 day, plus a sensing definition.

**Recommendation: B, keeping the litter by condition.** One call goes with it: **does the
2–6 band count dependent pups?** If it does, B needs litters of 2 or fewer (or 3 adults)
to stay in band all year.

### 2.3 Seedporter: fruit access before predation

**Now** [checked, err.log `vine:` and `forage:` lines across 8 seeds]:
- The vine sets 69–82 fruit units a year per world. 66–78 of them fall, and seedporters eat
  2.9–5.2 (4–7 %).
- Fruit is spread thin. The mean per bunch is 0.000076 across about 672,000 bunches the
  floor can reach, and 0.00076 on bunches that were visited.
- Seedporters are hungry 98 % of their time, see a fruit bunch 96 % of it, and meet a low
  offer 85–92 % of the time.
- Births stopped after hour 0, and no native reached adult.
- The Veilhound took 151 seedporters (60 of them adults), against 168 born.

- **A. Fruit in clusters.**
  - Mechanism: the same fruit set into far fewer, fuller bunches (fruiting spurs on mature
    or lit vine), so a bunch is worth the climb. A world fix: thin, even fruit is a supply
    with nothing worth moving for.
  - Budget [hyp]: if seedporters ate 20–30 % of the same set (15–25 units against about 4
    today), intake would rise about 2.5–4×, from 3.5× upkeep to roughly 9–14×. That should
    lift fullness off zero and let adults breed.
  - Cost: a flora change (about 0.5–1 day) plus knobs.
- **B. Fallen fruit stays edible on the ground for a while (windfall).**
  - Mechanism: seedporters and others forage the floor beneath the vine, and the ground
    exposes them to the Veilhound: a real risk-for-food trade.
  - Budget [hyp]: the fallen 66–78 units are 15–20× what is eaten now. Even 10 % would double
    intake. Predation rises with ground time.
  - Cost: about 0.5 day (fruit as a ground food stock with decay).
- **C. A refuge from the Veilhound.**
  - Mechanism: fewer grounded encounters. The Veilhound already takes only grounded
    seedporters.
  - Budget: removes 0.11/h of deaths but leaves births at about 0.01/h. N still falls.
  - Cost: a tweak.

**Recommendation: A** (B as an alternative if Wrysk prefers windfall). Predation should
fall by itself once fed seedporters stay up in the canopy [hyp].

### 2.4 Lanternjaw maturation

**Now** [checked]:
- Hatchlings ride a host, taking 0.09 of host body per world day [`parasite_take_per_day`],
  until they reach 0.24 of adult size [`parasite_detach_body_fraction`]
  (`lib.rs:2277-2286`; release in `drop_off_grown`, `step.rs:553`).
- Adult means body ≥ birth body, which is 0.6 of body max (0.021 / 0.035 at ×1,
  `body.rs:2030-2037`).
- Riders on littershredders (small hosts) sit just below the drop-off line for 9 h.
- Free juveniles grow 0.025/h (median). From about 0.25 to an adult's 0.63 takes about 15 h
  at shipped mass.
- That breaks the code's own rule, maturation at most a quarter of median life
  (`body.rs:142-150`; 24 h life → 6 h).
- Free juveniles are hungry (fullness p50 0.08). The Veilhound ate 29 juveniles.

- **A. Adult at a smaller size** (the birth-body fraction).
  - Mechanism: a small predator matures within the year, as weasels do.
  - Budget: at 0.35 of body max, adult is about 0.37 (×30), about 5 h from drop-off at
    0.025/h. Adults would then appear in most seeds in year 1 [hyp].
  - Cost: config only.
- **B. Hosts that can grow a rider.**
  - Mechanism: the rider's take scales with the rider's own need, or hatchlings attach only
    to hosts big enough to feed them.
  - Budget: frondgrazer-hosted riders drop at about 2.1 h, but still need about 13 h of free
    growth. On its own this isn't enough.
  - Cost: about 0.5 day.
- **C. Cover for juveniles.**
  - Mechanism: juveniles shelter in the dark or under cover, out of the Veilhound's reach.
  - Budget: removes 0.067/h of deaths but doesn't produce adults.
  - Cost: about 0.5–1 day.

**Recommendation: A, then B.** A meets the quarter-life rule. B stops the shredder host from
being a dead end.

### 2.5 Capgnawer and littershredder breeding

**Now** [checked]:
- Egg-layers pass these gates once eligible, in `lay` (`step.rs:4653`):
  1. 549 s of surplus;
  2. the 1,098 s interval;
  3. waiting up to 1,800 s for a full clutch;
  4. not on a wall;
  5. an egg site;
  6. no more than 8 of their own kind (bodies or eggs) within 1 m [`nest_crowded_at`,
     `step.rs:5171`].
- Refusals are counted [`nest_refusals_by_founder`] but the census doesn't print them.
- The capgnawer's egg site is within 0.5 m of a glowcap holding caps
  (`terrarium-wide.toml:111-112`). It eats 2 % caps, and four seeds laid no capgnawer eggs.
- Littershredders sit at 0.11 m from their nearest kin, with 65 % eligible. Births are about
  0.1/h against roughly 1–2/h that the timers allow [hyp: estimate from the timers].
- Both die mostly of age (median life 12 h, 1.2 years).

**Capgnawer.**
- **A. Caps as its real food.**
  - Mechanism: litter yields it little, so it lives at the glowcaps, where its eggs may go,
    and it answers glowcap abundance (Wrysk's mushroom example).
  - Budget [hyp]: its births would follow glowcap stands (8–104 per world). Seeds with
    about 8 stands may starve it out.
  - Cost: config (yields), plus checking that it can find glowcaps.
- **B. Eggs in litter near any fungus.**
  - Mechanism: relax the egg site.
  - Budget: it would breed like a second shredder and still not eat caps.
  - Cost: config.
- **C. Leave it.**
  - Persistence stays bimodal: 3–6 animals in 4 seeds, 18–58 in the rest.

**Littershredder.**
- **A. Patchy litter.**
  - Mechanism: litter decays fast in the open and builds under canopy, so shredders spread
    out and the crowded-nest brake stops binding. A world fix: "litter lies everywhere" is
    Wrysk's own example.
  - Cost: a flora change.
- **B. The brake counts eggs only, not adults.**
  - Cost: config or a small code change.
- **C. Measure first.**
  - Mechanism: print the refusal counter in the census and confirm which gate binds.
  - Cost: about 1 h.

**Recommendation:** capgnawer A; littershredder C first, then A. The brake is a hypothesis
until the counter is read.

### 2.6 Persistence counted absolutely, not "no worse than baseline"

**Now** [checked]: `score.py:777` calls an arm feasible when it loses no required lineage the
baseline kept. A shared failure therefore passes.

- **A. Absolute.** Every required lineage has adults or viable eggs at year end.
- **B. Both.** Relative for screens, absolute for acceptance.
- **C. Keep relative.**

**Recommendation: B.** Cost: about 1 h in `score.py`.

### 2.7 The year score credits hatchlings as recruits

**Now** [checked]: J counts surviving recruits, which include unmatured hatchlings
(`score.py:293-306`, `:466-467`).
- Lanternjaws score 82 survivors with 1 egg laid in 80 world-hours.

- **A.** Count only recruits that reached adult.
- **B.** Count completed replacement: native adults plus eggs laid by natives.
- **C. Keep.**

**Recommendation: A.** Cost: about 1 h.

### 2.8 The bellwing ceiling

**Now** [checked]:
- `fitness-v3.toml` sets the nominal at 90 per d72 (about 300 wide) and the ceiling at 900
  per d72 (2,970 wide).
- Bellwings end at 211–872. No seed is near the ceiling; 4 of 8 are above about 300.

- **A.** Ceiling at 3× nominal: 270 per d72, about 900 wide.
- **B.** A seasonal envelope: the year median at most 2–3× nominal, peaks free.
- **C. Keep 10×.**

**Recommendation: B**, which respects his seasonal swings. Cost: about 1 h.

### 2.9 Should hunts H count small prey?

**Now** [checked]: H counts every Veilhound and lanternjaw kill. 68 % of Veilhound kills are
bellwings.

- **A.** Count only prey above a share of the hunter's mass (for example 0.25).
- **B.** Biomass taken per hour, instead of a kill count. Outcome-shaped, and it needs no
  threshold.
- **C. Keep.**

**Recommendation: B.** A separate world question for Wrysk: is a ground hound snapping
perched flyers plausible at this rate? Cost: about 1 h.

### 2.10 Splitting `mass_small`

**Now** [checked]: `eco.toml` ties the seedporter, bellwing, capgnawer and lanternjaw to one
mass gene.
- The bellwing doesn't need it. The other three are below band, and two are declining.
- The lanternjaw's prey must be at most 1.5× its body. Its kills are mostly bellwing (199)
  and ripple-snail (131).

- **A.** `mass_bellwing` with the lanternjaw following it, plus `mass_fruit_cap` (seedporter
  and capgnawer).
- **B.** Four separate genes.
- **C. Keep.**

**Recommendation: A, after §2.3–2.5** (those roles change what these masses mean). Cost:
`eco.toml` only.

### 2.11 Woods (flora)

**Now** [checked]: wooded floor at year 1 is 260–711 forest faces, about 0.4–1.1 % of floor,
against 20–30 %. No fauna gene reaches it.

- **A.** A separate flora thread. Judge the fauna search on the wide year without W and G_v.
- **B.** Expect browsers to open the woods [hyp: offtake alone won't make woods].
- **C.** Revisit the target.

**Recommendation: A.**

## 3. Probes

**P6 (life-history code read)** is done: the citations are in §2.1–2.5.

**P1. The energy surplus as a proxy for births** [checked: `scratchpad/eco-roles/p1.py`, 8
seeds, minutes 60–600].
- Method: per-capita surplus (intake − upkeep − motor) in one window against per-capita
  births in a later window. Each cell gives r pooled over seeds / r within seeds.
- **It misses the pass bar of r > 0.7 for the fast lineages:**

| window, lag | frondgrazer | loftstrider | seedporter | capgnawer | shredder | bellwing |
|---|---|---|---|---|---|---|
| 30 min, 0 | 0.18/0.12 | 0.26/0.18 | 0.28/0.26 | 0.45/0.30 | 0.48/0.49 | 0.49/0.49 |
| 30 min, +1 | 0.38/0.34 | 0.34/0.28 | 0.32/0.30 | 0.51/0.47 | 0.42/0.44 | 0.68/0.68 |
| 60 min, 0 | 0.36/0.30 | 0.40/0.29 | 0.40/0.40 | 0.62/0.55 | 0.65/0.67 | 0.57/0.57 |
| 60 min, +2 | 0.82/0.83 | 0.53/0.45 | 0.20/0.14 | 0.66/0.67 | 0.43/0.48 | 0.37/0.38 |

- The frondgrazer's 0.82 at +2 h is −0.08 at +1 h, so it is noise.
- The lanternjaw and ripple-snail are near zero or negative. Lanternjaw births are dusk
  hatches from the egg bank.
- Most of the surplus goes into juvenile growth, so it doesn't predict births.
- The **eligible share** in the same window works better for live-bearers: frondgrazer 0.97,
  loftstrider 0.92, seedporter 0.97, bellwing 0.73 (60 min).
- For the egg-layers it is negative: capgnawer −0.62, shredder −0.53. This fits a brake
  after eligibility (§2.5).
- **Implication [hyp]:** the drift readout should report births and deaths directly over
  1–2 h windows, not an energy-flux proxy.

**P2 and P3** are running detached:
- binary `774bd98c`, from s1 and s7 at `m000600` (s1 is short on food, s7 food-rich), 45
  sim-minutes each;
- outputs in `runs/eco-roles-probes-2026-10-09/`.
- **P2:** frondgrazer mass ×1, 4, 16 and 64, tied to the Veilhound as `mass_grazer` is.
- **P3:** gut throughput S = 1/3 and 1/6. S = 1 is P2's ×1 arm.
- Results to be added here.
