---
design_status: proposal
last_reviewed: 2026-10-02
decision_refs: []
---

# One biological clock: night, metabolism and predator cadence

A proposal by Fable, written for Wrysk's decisions. Astra reviews it first. Every number here
comes from `design/7_Research/timescales-inventory-2026-10-02.md` (written "the inventory"
below) unless it says otherwise.

## The problem in one line

**The world's biology runs about 2,000–5,000× faster than nature, but its day runs only 12×
faster.** So a 40-minute night is, metabolically, about 80 days without food.

Every symptom Wrysk raised follows from that mismatch:
- **Night.** The night is 39.6 min. Seven lineages hold 8.3 min of reserve, and their sleep
  breaks at hunger 0.5, so they can sleep at most 0.14 h from full.
- **Real time.** A 12 h night would be about 1,000 days of metabolism.
- **Predator cadence.** A lanternjaw must kill 0.9–1.3 frondgrazers a day to break even. A
  chorister pack of 4 must kill 23.5 a day. The whole gate saw 1 kill in 48 world-days.
- **Founders.** They arrive at half reserve and starve before the first night (7 of 9
  lineages by minute 15–46).

## The principle

A world day should feel like a day to its animals:
- **A sleeping animal outlasts its sleeping phase with margin.** That is the night for
  diurnal lineages, and the light hours for nocturnal ones.
- **A predator's meal interval is about a day or longer.** Ambushers go longer, packs about
  a day.
- **A life spans several days,** and first breeding comes after at least one full day and
  night.
- **Plants smooth the night.** Income stops in the dark, but growth continues from what the
  day stored. That is how real plants behave: leaf starch is drawn down linearly to about
  95 % by dawn (Graf et al. 2010).

The search can then tune within these bounds instead of around them.

## Part 1: make the compressed day consistent (recommended now)

The 120-min cycle stays the ecological day: every preset and every training world use it.
These changes go in before the full-roster search. Most are knobs; two are mechanics.

### 1a. Animals sleep through the night

- **Sleep holds until near starvation.** Raise `wake_hunger` from 0.5 to about 0.9. Today
  hunger wakes a sleeper after 0.14 h.
- **Sleep and torpor depth per lineage**, anchored on Geiser 2004 and Ruf & Geiser 2015:
  daily torpor about 35 % of BMR, hibernation about 6 %.
  - Endotherms: sleep 0.5 (today's value).
  - Ectotherm-like lineages: sleep 0.3. These are the shredder, ripple-snail and capgnawer,
    and also the lanternjaw, an ambusher.
  - A lineage-level field already exists (`sleep_upkeep` in `body.rs`); it becomes a
    registry row per lineage.
- **A night-endurance screen** in the search's arithmetic pre-screen, beside R/S/F:
  `reserve_cap / (maintenance × sleep_upkeep) ≥ 1.5 × the lineage's sleeping phase`.
  - Diurnal lineages: 1.5 × 40 min = 60 min.
  - Nocturnal lineages: 1.5 × 80 min = 120 min.
  - Today's values are 17 min, or 28 min at sleep 0.3. So upkeep or reserve must move by
    about 3–7×. The search finds which; the screen refuses anything short.
- **Founders arrive with a full reserve** (today half): a preset knob.

### 1b. Plants spend the day's store at night (mechanics)

- **Foliage reflush from reserve** continues in the dark, at a rate set by
  `reserve ÷ time to dawn`: the starch rule.
  - Today `flush_light` = daylight gates the reflush, so no foliage is built at night.
  - The store is paid from day income only, so a day's total energy is unchanged; only its
    timing smooths.
- **Nectar and fruit** are made at night from reserve at `night_bloom_scale` (proposed 0.3).
  Today they stop at daylight 0. Many real flowers secrete at night, for moths. This keeps
  the bellwing and seedporter from facing a 40-min famine on top of their own sleep.
- **No moonlight income.** Real moonlight is about 1/380,000 of the sun (Kyba et al. 2017),
  and the store does the same job truthfully. The moon stays picture-only (GPU
  `MOON` = 0.3).
- **Glowcap** stays the world's light-free producer: a saprotroph, already not night-scaled.
  It feeds the nocturnal capgnawer.

### 1c. Predator cadence is a target, not an accident

- **Lanternjaw (nocturnal ambusher).** Hunt at dusk and through the night. After a kill it
  rests satiated in torpor (already built: `torpor_upkeep` 0.25, `rest_at`/`wake_at`).
  - **Target meal interval: 1–3 days.** Real ambushers feed every 10–40 days and active
    small predators daily (Secor), so this is compressed but in the right order.
  - Arithmetic: break-even ≤ 0.5 grazer kill per day, against 0.9–1.3 today. Lanternjaw
    upkeep comes down about 2–3× within its gene range.
- **Chorister (pack).** Hunt at dawn and dusk; the `Crepuscular` rhythm exists, unused.
  Rest at the den by day and night.
  - **Target: one kill per pack per 1–2 days.**
  - Today's need, 23.5 grazers a day for 4 adults, is about 20× too high. Three levers, all
    knobs: chorister upkeep ÷ ~5, a smaller pack (2–3 adults), and a pack prey cap that
    admits the loftstrider.
    - Packs take larger prey: `prey_max_ratio` × √pack size or similar.
    - One adult loftstrider carcass is about 4 ou assimilated.
- **Balance screen** (arithmetic, per candidate, before any run):
  `Σ predators × break-even kills per day ≤ 0.5 × the prey births per day expected at the
  prey band`.
  - With the gate's prey births (about 10–15 a world-day) and lanternjaws at 0.5 kill a
    day, a world sustains about 10–14 lanternjaws. That is inside the 5–15 % target at about
    150 animals.
  - Today's lanternjaw sustains 3–4, under 3 %. **This is why the target was unreachable.**
- **Births.** Prey births a day must cover predation plus the other deaths at the band;
  the screen above and the existing breed targets cover that. Predators breed rarely: the
  lanternjaw's egg bank and dusk hatch, and the chorister's one litter per 6 h, both stay.

### 1d. What Part 1 leaves as it is

Life histories stay fast:
- lifespans of 4–12 h, which is 2–6 days;
- first breeding at about 1 h;
- training horizons of 2 h, which still include one full night.

## Part 2: real time (Wrysk's choice)

Under `local` mode the night is 12 h. No compressed biology survives that honestly. There
are three coherent ways to have a real-time sky:

| | what runs on the wall clock | biology | what you see | cost |
|---|---|---|---|---|
| **RT-A: sky only** | the picture: sun, moon, light | stays on its own 120-min ecological day | the real sky, but the animals keep their own short days (sleeping grazers at real noon, lanternjaws hunting in daylight) | none; Part 1 unchanged |
| **RT-B: slow world** | sky and ecology | one `biology_time_scale` knob divides every biological rate by 12 (upkeep, intake, growth, plant rates, gestation, lifespans); speeds stay real | a real day: births a few times a day per lineage, hunts at dusk, lives of 1–2 real days | it can't be searched directly (horizons of days). Transfer by the scale, then one retune and a 2–3-day soak on eidolon. Space isn't scale-free: animals walk 12× farther per metabolic day, so encounter rates rise and the retune is needed |
| **RT-C: moonlight + torpor** | sky and ecology | plants earn a moonlight share (5–10 % of day); every animal torpors at 5–10 % basal all night | the real sky, a mass torpor at night, and whole generations living inside one real day | cheap, but the least true: 380,000× too much moonlight, and life is shorter than a day |

**Fable's recommendation:**
- Ship RT-A first, as the meaning of `local` mode.
- Keep `cycle` as the default, because it carries the activity: dusk hunts, morning grazing,
  night sleep, all in a 2-hour loop.
- Try RT-B as a soak experiment if a real-day piece is wanted.
- Don't build RT-C.

## Part 3: what changes in the full-roster search (step 3 of the plan)

- **Genes:** the six newcomers get the R3 per-lineage genes: upkeep, motor, bite, yield,
  surplus floor, lifespan, incubation or gestation, and growth. Plus:
  - the per-lineage sleep depth;
  - the chorister pack size and pack prey ratio;
  - `night_bloom_scale`.
- **Screens (refuse before running):**
  - R/S/F;
  - night endurance (1a);
  - predator balance (1c);
  - nectar and fruit supply against the bellwing's and seedporter's need. A seedporter needs
    2.6 ou a day against at most 0.72 ou a day per lanternberry, so at least 4 fruiting stands
    per seedporter.
- **Targets:** as today:
  - every lineage persists and every non-exempt lineage breeds in the second half;
  - the band;
  - browsers move;
  - lanternjaw 5–15 %;
  - plus the chorister: at least one litter per world, and packs persist.
- **Horizons stay 2 h**, so each world sees one night. Founders arrive at full reserve.
- **The brains stay frozen during this search:** C1's centres for the two trained lineages,
  heuristics for the rest. The lock step follows.

## Decisions for Wrysk

1. Part 1, a consistent compressed day: sleep through, sleep depth, plant night store, night
   bloom, the two screens, full founders. **Recommended.**
2. Real time: RT-A now (recommended), RT-B as an experiment, or RT-C.
3. Predator cadence targets:
   - lanternjaw: 1 meal per 1–3 days, hunting at dusk and night;
   - chorister: one kill per pack per 1–2 days, hunting at dawn and dusk (crepuscular).
4. Later (not now): stretch life histories toward "several days to adulthood". That needs
   training horizons of 3+ days, about 3× the compute.

## Sources

- The inventory: `design/7_Research/timescales-inventory-2026-10-02.md` and its citations
  (Geiser 2004; Ruf & Geiser 2015; Graf et al. 2010; Kyba et al. 2017; Secor; Lindstedt &
  Boyce 1985).
- `design/7_Research/ecology-parameter-audit-2026-09-28.md` §2: compression factors.
- `design/cotraining-plan-2026-10-01.md`: targets and the 2026-10-02 gate.
