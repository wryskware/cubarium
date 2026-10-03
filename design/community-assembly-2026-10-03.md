---
date: 2026-10-03
author: Fable
status: proposal (not decided; for Astra's review, then Wrysk)
---

# Community assembly: staged arrival, and where training starts

## Why

Wrysk, 2026-10-03:
- "it might not be the best data to try to balance choristers coming in on a world with only
  the founders, since it should ideally take a larger population of animals to sustain them.
  and similarly with loftstriders … they need groves of vaulttrees already established."
- "bellwings might need to establish later as well when there is more latticevine."
- "i wonder if our grid search or training runs could benefit by starting from 2hr established
  worlds, or from much richer founder setups."

Today every lineage is founded at minute 0 on a bare world with founder plants. After the gut,
a 2 h run is one world day, and lifespans are 6–16 world days. Choristers, lanternjaws and
loftstriders are readouts only in the search until their food base exists (main 72a732a7).

## Part 1: Staged arrival

**A lineage arrives when the world can feed it, not on a clock.**

1. **Readiness per lineage `L`,** from its own diet. These are the same units and the same
   arithmetic as the search's food screens (`crates/cubarium-search/src/es/voxel/live/screen.rs`),
   but read from the live world state, not from parameters:
   - **Supply** `Σ_food production rate of the foods L can reach`:
     - mouth band for foliage;
     - flight for nectar;
     - climbing and gliding for fruit;
     - water for film;
     - the litter layer.
   - **Demand** `N_arrive × f_L × body_L`.
   - **Ready** when `supply ≥ margin_L × demand`, held for `sustain_s`. This is hysteresis,
     so a passing transient doesn't trigger an arrival.
2. **Predators** (chorister, lanternjaw): supply is **prey production**, the prey biomass
   born and grown per world day, not the standing prey.
   - Ready when `prey production ≥ margin_pred × predator demand`.
   - A predator taking 10–30 % of prey production is a common bound, so `margin_pred` ≈ 3–10.
   - The lanternjaw's dormant egg bank already waits for footfall and hosts. Staged arrival
     decides only when the bank is buried.
3. **The order emerges from the diets.** Expected:
   - plants first;
   - then the foliage, litter, glowcap and film feeders (frondgrazer, littershredder,
     ripple-snail, capgnawer);
   - then the fruit and nectar feeders (seedporter, bellwing) as latticevine colonies and
     bloomcrowns establish;
   - then the loftstrider once vaulttree groves are in reach;
   - then choristers and lanternjaws once the prey base produces enough.

   No stage table is coded.
4. **Arrival:**
   - `N_arrive` founders arrive at the place with the most reachable supply, at its nearest
     world edge or wall.
   - Live, it's a visible event, which also gives the hunt-event cue work a hook.
   - The presentation of an arrival is Wrysk's, like the look.
5. **Knobs (registry rows):**
   - `margin_L` per trophic level;
   - `N_arrive` per lineage;
   - `sustain_s`;
   - `max_wait_s`, after which a lineage that never became ready is reported, not forced.
6. **Re-arrival (immigration) after extinction:** a knob, **off in training**, where
   extinction is the signal. For live, it's Wrysk's call.

## Part 2: Where training starts

### 2a. Grown worlds, for brain training (ecology fixed)

- **Grow each seed once,** through staged arrival, until every lineage has arrived, plus one
  world day. Save it with the existing layered snapshot (`voxel_census save=` / `load=`;
  `world.bin`, `flora.bin`, `fauna.bin`). Evaluate every candidate from a copy.
- **The library belongs to a build.** A schema bump refuses old saves (always fresh, never
  migrate), so the library is regrown overnight after each bump.
- **Cost:** about 1–3 world days of growing per seed, once, for any number of candidates.

### 2b. Computed founder setups, for the ecology search and the grid

A world grown under the default parameters sits in balance with the defaults. Starting a
candidate with different plant output or metabolism there biases results toward the defaults.
Instead, **found the world at the composition the candidate's own parameters predict:**

- **Plants** at the density the candidate's productivity holds, per habitat, including mature
  vaulttree groves and latticevine colonies on walls. This needs a "grown placement" in the
  plant seeder: mature stands at a given density and a given age mix.
- **Consumers** at a fraction `φ` (default 0.5) of their carrying capacity
  `K_L = supply_L / (f_L × body_L)`, with a mix of ages and sizes.
- **Predators** at the number prey production supports at `margin_pred`.
- **Then relax** for `T_relax` (default 1 h) and score the rest of the run.

The arithmetic that sets the readiness thresholds also builds the founder setup and the
search's food screens, so one module serves all three.

### 2c. What the grid uses

- **Until 2b exists,** the grid (`voxel-ecology-grid`, being built) uses today's founding.
- **Its readout is the balance measures** (B, R̂, access, sated share), not alive counts.

## Part 3: What has to be measured first

1. **Vaulttree grove maturation**, plants only. A soak is running: eidolon, 4 seeds × 16 h,
   `nofauna`, saved at the end.
2. **Latticevine colony growth on walls**, after the vine fix (package `plants3`).
3. **The time to readiness** for each lineage on default parameters. This sets `max_wait_s`
   and the length of the grown-world spin-up.

## Questions for Astra

1. Is readiness from diet arithmetic sound, or does it hide a failure mode? For example: supply
   is reachable but contested; patchy supply; a lineage that's ready but whose arrival site is
   a sink.
2. Should predator readiness use prey **production** with `margin_pred` 3–10, or something
   else?
3. Does 2b remove the default bias, or replace it with a model bias from approximate
   equilibrium arithmetic? Is a 1 h relax enough?
4. Should the ecology search **evaluate the assembly path** (start bare, stage arrivals), or
   start established (2b)? The first tests whether the world can build itself. The second
   tests whether it can hold.
5. What would make a staged-arrival world fail to reach a full community at all, and how
   should that be scored?
