---
design_status: proposal
last_reviewed: 2026-10-03
decision_refs: []
---

# Realistic metabolism and a minimal gut

A proposal by Fable. This is **revision 2**: the gut version. It answers Astra's review
`design/7_Research/astra-metabolism-meals-review-2026-10-02.md`, findings [M1]…[M11].

## What Wrysk asked for

On 2026-10-02 and 10-03:
- Food should be filling.
- Animals eat a few meals a day and take breaks between them.
- They sleep through the night "because you're still digesting and because you have
  reserves".
- **The gut, not a coded meal rule,** should make meal-then-stop happen.
- Hunger should also respond to the fat reserve, "but maybe that's overcomplicating the first
  pass".
- **The test that matters:** the expected behaviour has to drop out, for example "choristers
  only need to eat one big meal every 1–2 days".

## The faults

1. **Basal upkeep is about 100× too high.**
   - Seven lineages burn 0.001 of their body per second. At mass scale 30 that is 7.2 bodies
     per 120-min world day.
   - Real field metabolism is about 2–10 % of body per day [M1]. The comparison is with
     mammals; ectotherms run lower.
2. **Intake goes straight into the reserve,** sized by the current hunger. So animals nibble
   continuously, and there are no meals.

## The model

### 1. Daily need per lineage

`maintenance_per_s = f / 7200 s`. Motor stays tied to basal by its existing ratio.

| lineage | `f` | sleep | rhythm-aware daily spend `E` |
|---|---|---|---|
| bellwing | 0.15 | 0.5 | 0.126 |
| seedporter | 0.08 | 0.5 | 0.067 |
| frondgrazer | 0.06 | 0.5 | 0.050 |
| loftstrider | 0.04 | 0.5 | 0.034 |
| chorister | 0.08 | 0.5 | 0.067 |
| lanternjaw | 0.04 | 0.3 | 0.021 |
| littershredder | 0.03 | 0.3 | 0.016 |
| capgnawer | 0.03 | 0.3 | 0.016 |
| ripple-snail | 0.02 | 0.3 | 0.010 |

- `f` is the fraction of adult body burned per world day, awake and at rest.
- **`E` counts sleep:** about 68 % of the day awake and 32 % asleep for diurnal lineages, the
  reverse for nocturnal ones [M3]. It is basal only; walking, breeding, provisioning and
  luring add to it, so they are reported separately.

### 2. The stomach

Each body has a stomach `S`, holding raw food in organic units [M2]. Its capacity is
`K = k × current body` [M9], so a juvenile has a juvenile-sized stomach.

- **Eating:**
  - Bites add raw food to `S` at `b × current body` per second.
  - They are capped by the stock, the mineral budget and the room `K − S`.
  - **A full stomach stops eating physically.**
- **Digestion:**
  - `S` empties at `S / τ_food` per second.
  - The digested food times the existing diet `yield` goes into the reserve; the rest takes
    the existing dung and litter path.
  - **Digestion runs only while the reserve has room.** A well-fed animal's stomach stays
    full until its body has burned room for it. That is why meals are spaced: no rule spaces
    them.
  - Stomach content is in the books, so organic matter is conserved.
- **Appetite, the only behavioural trigger:** a body *wants* food when
  `S ≤ s_start × K`, where `s_start = s0 + (1 − s0) × max(0, (h − 0.5) / 0.5)`.
  - `h` is the reserve hunger.
  - A fed animal starts at a nearly empty stomach (`s0`, default 0.15).
  - A depleted one eats whenever there is room. That is Wrysk's fat-reserve term, kept to
    one line.
  - It wants food until the stomach is full or food runs out.

**Meal interval for a non-breeding adult** (the only cadence claim [M3]):

`interval ≈ (1 − s0) × k × yield / E` world days.

Each meal fills `k`; the body burns `E` a day; digestion runs only into the room that
burning makes.

| lineage | food | `k` | `τ` | interval (yield 0.5) | target |
|---|---|---|---|---|---|
| bellwing | nectar | 0.03 | 30 s | 0.10 day: about 10 meals a day | many small sips |
| seedporter | fruit | 0.05 | 60 s | 0.32 day: about 3 a day | a few |
| frondgrazer | foliage | 0.06 | 120 s | 0.51 day: about 2 a day | a few grazing bouts |
| loftstrider | foliage | 0.05 | 120 s | 0.63 day: about 1.6 a day | one or two long browses |
| littershredder | litter | 0.02 | 120 s | 0.53 day: about 2 a day | a couple of feeds |
| capgnawer | caps | 0.02 | 120 s | 0.53 day | a couple |
| ripple-snail | film | 0.012 | 120 s | 0.51 day | a couple |
| **chorister** | meat | 0.20 | 300 s | **1.27 days** | Wrysk: one big meal per 1–2 days |
| **lanternjaw** | meat | 0.15 | 300 s | **3.0 days** | one meal per 2+ days |

- A wolf eats about 20 % of its body at a kill; ambushing snakes take large meals rarely.
- **Bite rate:** `b` is set so a meal takes about `k / b` ≈ 1–2 min for grazers, 20–40 s
  for nectar and 2 min at a carcass.

### 3. What eats, and when (one precedence) [M7]

1. **Off-phase:** the body sleeps, whatever its stomach holds.
2. **Emergency:** a sleeper wakes only if its reserve hunger reaches `wake_hunger` (0.9). It
   then stays awake and eating until its stomach is full **and** its hunger is below 0.6, or
   its own phase begins. This latch replaces `resleep_hunger` and ends the flicker.
3. **Alert:** unchanged.
4. **On-phase, wanting food:** it seeks food and eats.
   - Hunters may lure, target and strike **only** in this state.
   - Pack target acquisition reads the pack's members.
   - Flyers' sipping uses the same gate.
5. **On-phase, not wanting food:** it doesn't bite and doesn't hunt. It rests, wanders or
   explores, which the curiosity-trained brains already like.

This removes the hunters' satiated rest (`rest_at`, `wake_at`, `torpor_upkeep` between
meals) [M3].

### 4. Predators and young

- **Packs [M6].** "One big meal per 1–2 days" is the **non-breeding pack** target.
  - Choristers provision pups **from the stomach first** (regurgitation, as wolves do), then
    from reserve above the 60 % floor.
  - A pack raising a litter therefore empties its stomachs sooner and hunts more often. That
    is expected and tested separately.
  - At a carcass each adult takes up to its stomach's room, about 0.55 ou raw. The rest stays
    carrion, with its existing about 1.4 h e-fold decay. A pack that wants food may return to
    leftovers rather than kill.
- **Lanternjaw riders [M8].** Rider income is re-tied from the host's basal (now tiny) to the
  host's body.
  - The rider takes `parasite_take_per_day` × host body per world day, from the host's
    reserve, then from its body.
  - The default is sized so a rider on an adult frondgrazer reaches drop-off in about 1–1.5
    world days.
  - On a littershredder (a quarter of the body) it is about 4× slower. Host size then matters
    without a host-size rule.
  - The host pays, and eats more often.
- **Juveniles and breeders [M4, M5].**
  - Growth and births still draw on the reserve. Juveniles and breeders refill more often:
    the young graze nearly continuously, and pregnant animals eat more.
  - Digestion throughput (`k × yield / τ` per second) now limits growth.
  - **Juveniles grow slower:** about 1–2 world days to breeding size, against 44 min today.
    This is the one life-history change, and it is the cost of real meals. Faster growth
    would need faster digestion, and then there are no meals.

## What changes elsewhere

- **The plant–animal balance [M10].** Animal demand falls 50–360×, so plants are relatively
  plentiful. Populations are bounded by births, lifespans, predators and meals, not by
  starvation.
- **The roster search is re-based:**
  - upkeep genes are ×1 at `f`;
  - new genes: stomach `k`, digestion `τ` per food, `s0`, `parasite_take_per_day`;
  - bite rate becomes the intake gene;
  - the night screen stays as a guard;
  - R/S/F stays a readout or soft;
  - the big-lever grid (master metabolism × plant productivity) runs first.

## Acceptance (tests first, then measure) [M11]

1. **Cadence:** for each lineage, a non-breeding adult on ample food under its own rhythm.
   - **Pass:** meals per world day within ±30 % of the formula, counted over 3 world days.
     The chorister and lanternjaw are counted over 6.
2. **The stomach caps a sitting:** a starved adult eats at most `K` per sitting; famine
   recovery takes several meals.
3. **A full stomach on food takes no bite.**
4. **Sleep:**
   - A fed diurnal adult sleeps through the night, and a fed nocturnal one through the day.
   - A starving sleeper wakes, refills, and doesn't flicker (≤ 2 sleep flips per hour).
5. **Hunters:** a chorister or lanternjaw with a full stomach starts no lure, target or
   strike. With an empty stomach, it does.
6. **Packs:**
   - A non-breeding pack's next kill attempt comes at the formula's interval, ±30 %.
   - With pups, provisioning draws on the stomach first, and the pack hunts sooner.
7. **Riders:** an adult-grazer host brings a rider to drop-off in 1–1.5 world days, ±30 %.
   A shredder host is slower, and the host's spending rises by the take.
8. **Juveniles:** a frondgrazer reaches breeding size in 1–2 world days with food in reach.
9. **Books:** food eaten = assimilated + dung or litter + Δstomach, to the existing
   tolerance.
10. **Measure:** desktop terrarium, seeds 1–8 × 2 h, plus one 8 h run for predators and
    riders. Report per lineage:
    - meals per day;
    - stomach fullness over the day;
    - sleep wakes;
    - alive;
    - births;
    - kills;
    - rider growth.

## Not now

- Food-type quality (dense nectar against bulky foliage beyond `τ`).
- Visible dozing after a meal.
- Hunt-event cues.

## Repair 2: the implementation spec (answers Astra's repair-1 check, [M1]–[M12])

Astra's repair-1 status was "revise". It accepts the minimal gut and asks for executable
detail. This section **overrides** any line above that it contradicts. The build package
implements this section, and Astra reviews the implementation, not another draft.

1. **The diagnosis [M1].** The 50–360× is the cut to the **basal reference rate** only.
   - Growing, breeding, parasitised and food-starved animals still spend more.
   - Starvation still limits a population whose food is out of reach.
   - The 2–10 %/day figure is an analogy to mammals, not a calibration.
   - Today's defaults and the live preset's values are reported separately.
2. **Digestion [M2].** Per tick, the raw food digested is
   `min(S, S·dt/τ, room / y_eff)`, where:
   - `room` = reserve cap − reserve;
   - `y_eff` is the effective, mineral-limited yield. At `y_eff` = 0 the stomach holds.

   The undigested food stays in the stomach.
3. **The eating latch [M2, M7].**
   - **On** when `S ≤ s_start·K`, in the states rule 7 allows.
   - **Off** only at a full event, `S ≥ 0.98·K`.
   - While it's on, the body seeks food and bites at food. Losing access to food doesn't clear
     it.
   - The latch is saved with the body. A newborn starts with it off and an empty stomach;
     a founder starts with it off and a stomach at `s0·K`.
4. **Bite rate [M4].** `b = 2k / τ` per unit current body. Then a meal fills in about
   `0.69 τ` with digestion running: about 83 s for a grazer, 3.5 min at a carcass. It is one
   gene per lineage, independent of upkeep; only motor stays tied to upkeep [M10].
5. **Juveniles and lifespans [M4, M11].**
   - Maturation is measured, not promised. The plan expects about 1.5 world days at the
     grazer's maximum digestion throughput (1.8 current bodies a day).
   - Lifespans rise so that maturation is at most a quarter of the median life. These are
     Wrysk's values, 2026-10-03, by size:
     - loftstrider: 16 world days;
     - seedporter, frondgrazer: 8 world days;
     - shredder, snail, capgnawer, bellwing: 6 world days;
     - lanternjaw, chorister: 12 world days.
   - The live medians are 1.22 world days (small) and 2.02 (lanternjaw).
6. **Provisioning [M6].**
   - **Regurgitation** moves raw stomach food from giver to pup stomach.
     - It is capped by the pup's room, and the food keeps its composition.
     - It needs the giver's `S > 0` and a pup that wants food. The giver's reserve floor
       does not apply.
   - **Fallback:** the existing reserve-to-reserve transfer, at 70 % efficiency above the
     60 % floor. It is booked separately.
7. **Precedence, executable [M7].**

   | | state | rule |
   |---|---|---|
   | a | safety and alert | awake, as today: submerged, falling, attached, flying, mid-strike, alerted |
   | b | emergency latch | **on** at reserve hunger ≥ `wake_hunger` (0.9), **off** below 0.6 |
   | c | ordinary off-phase | asleep |
   | d | on-phase | the eating latch on means seek, eat and (hunters) hunt; off means rest, wander or explore |

   - **While the emergency latch is on:**
     - The body is awake, even off-phase.
     - It may seek and eat when its eating latch is on. Hunters may lure, target and strike
       then too.
     - With a full stomach it stays awake but doesn't eat, waiting for digestion.
   - **Packs:** the pack starts a hunt when at least half its adults have the eating latch on,
     or any adult is in emergency. Members join if their latch is on or they are in
     emergency.
8. **Bouts [M9].** The stomach bounds its **contents** (`0 ≤ S ≤ K`), not a sitting's
   intake.
   - Juveniles, famine recovery and provisioning may feed in long continuous bouts.
   - Bouts are recorded by kind: normal, emergency, growth, provisioning.
9. **Riders [M8].** `parasite_take_per_day` defaults to 0.09 of host body per world day. It
   goes straight into the rider's reserve.
   - **Sizing:** on a 1.5-ou grazer, about 1.19 world days to drop-off (Astra's integration).
   - The host's spending includes the take. That is about 2.8× a grazer's basal, so infested
     hosts visibly eat more.
10. **The stomach's contents [M12].**
    - **Pools:** a small per-food-class store of raw organic matter, mineral and energy. Each
      class digests at its own `τ` and yield; there is no mixing rule beyond proportional
      transfer.
    - **Separate from egesta:** the existing `gut` is egesta and stays apart.
    - **On death or removal,** the contents go to the carcass or litter as raw matter, with no
      extra carcass yield.
    - **Saved state:** serialised, with a fauna schema bump.
    - **Books, for organic matter, mineral and energy:**
      food in + transfers in = assimilated + egesta + respiration + transfers out
      + Δstomach.
11. **Search rebase [M10]** (a second package, after the gut lands):
    - write the resolved new values into the live presets;
    - upkeep genes are ×1 at the new `f`;
    - new genes: `k`, `τ` per food class, `b`, `s0`, `parasite_take_per_day`;
    - the night guard counts digestible stomach stock, or is soft;
    - replenishment counts reserve plus digestible stomach, so moving food around is not
      income.
12. **Tests [M11].** Use controlled fixtures:
    - fixed adult body, breeding off, lifespan off, ample reachable food.
    - **Interval:** measure at least 3 completed meal intervals per lineage, against the
      formula with the unrounded `E` (Astra's exact intervals: 0.1012, 0.3162, 0.5060,
      0.6324, 0.5407, 0.5407, 0.4866, 1.2649 and 3.0415 days), mean within ±30 %.
    - **Separate tests:**
      - no prey, hungry: lures or seeks, but no strike without eligible prey;
      - emergency and alert precedence;
      - interrupted food access keeps the latch on;
      - famine recovery: `0 ≤ S ≤ K`, books close, bout duration recorded;
      - a juvenile reaches breeding size, with time measured against the 1.5-day expectation;
      - regurgitation conserves and keeps composition;
      - a rider's budget on a fixed host;
      - books close over a full day for organic matter, mineral and energy.
    - **Measure:** seeds 1–8 × 2 h for access and survival, and one 8 h world as
      exploration, not cadence statistics.
