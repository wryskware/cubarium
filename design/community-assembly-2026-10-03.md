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

---

## Revision 2 (after Astra's review, `design/7_Research/astra-community-assembly-review-2026-10-03.md`)

Astra said revise, with three blocking findings: a shared food budget, predator units, and
"never arriving" as an escape. This revision overrides Parts 1–2 above where they differ.

### R1. Two evaluation settings, scored separately [A5]

- **The established setting:** the ecology grid, the ecology search and brain comparisons.
  - Every candidate starts from the **same shared, paired ensemble** of starting states, not
    from candidate-specific ones.
  - Here all nine lineages are present and **all are scored**, including the chorister,
    lanternjaw and loftstrider. Their readout-only flags apply only to today's
    founders-only diagnostic.
- **The assembly setting:** a smaller staged-arrival arm from a bare world, run on
  finalists only.
- **Finalists must pass both.** The two results are reported separately and never averaged.
- **Today's founding** remains a disclosed early-community diagnostic. It doesn't rank the
  full community.

### R2. The starting ensemble [A4, A8]

Three levels, the same states for every candidate:
1. **E0, today's founding.** The diagnostic.
2. **E1, constructed rich.** Built by explicit construction, not predicted from a candidate's
   parameters:
   - mature vaulttree groves and latticevine colonies on walls, at fixed densities;
   - roughly 3× today's consumer founders;
   - one chorister pack sized to the prey present.

   All injected organic matter, mineral and energy is booked: wood, soil and film, reserves,
   stomachs, eggs. Ages, sizes and reproductive states are drawn as consistent sets: a
   juvenile is small and not gravid.
3. **E2, grown.** A staged-arrival run (R4) carried past the last arrival plus one world day,
   then saved. Hard seeds are kept; a seed where something never arrives stays in the panel,
   with its failure on record.

**Episode contract:** every evaluation loads a start through one initializer, which:
- matches the layer files, tick and phase, resolved parameters, and build and policy
  identities;
- applies the candidate's parameters over the loaded state;
- resets controller memory, followed by a fixed burn-in that is the same for every
  candidate;
- reads scores from checkpoint-relative ledger baselines.

Starts are disposable. They are regenerated when relevant code or config changes.

**Relaxation is judged by trends, not a fixed hour:** stores, producer stocks and recruitment
over phase-aligned windows. The startup interval is reported on its own.

Candidate-specific constructed starts may come later as a second treatment, never as the
only comparison.

### R3. One shared food budget [A1, A2]

Readiness and construction both use one local resource budget:
- Each source (stand, vine face, litter cell, glowcap, film, prey cohort) is allocated **once**
  across resident and prospective consumers.
- **Supply is realised:** a rolling, spatially indexed estimate of edible production, taken
  from source-level production, withdrawal and loss counters. Counters are added where the
  state lacks them. A standing-stock buffer is reported beside it.
  - The configured-rate estimates in the search screens stay as optimistic bounds, labelled
    as such.
- **Demand is assimilated:** the allocation converts at the eater's effective,
  mineral-limited yield, against rhythm-aware basal demand plus explicit allowances for
  activity and family (growth, reproduction, provisioning, riders).
- `K_L` is a provisional food-budget bound under that allocation. It is not carrying
  capacity.
- **Total bodies and placement attempts are capped,** so a low-upkeep candidate can't flood
  the world through the initializer.

### R4. Staged arrival [A1, A6, A7]

**The required roster** is all nine lineages. Each has an explicit state:
- pending;
- ready but unplaceable;
- arrived;
- established (recruiting from new food, not from an endowment);
- extinct;
- deadline missed.

**Rules:**
- **Readiness:** realised allocated supply ≥ `margin_on × demand` turns it on, and below
  `margin_off` turns it off (`margin_off < margin_on`). It must hold for `sustain_s`.
- **Deadline:** a lineage not arrived at `max_wait_s` is an **assembly failure**, with its
  reason: production, contested supply, missing habitat, den or host, placement, or failure
  after arrival. No forced immigration erases it.
- **A late arrival** starts its own post-arrival observation window.
- **A fresh egg bank** is not an established predator.

**Arrival units** follow each founding mode:
- bodies, via a new general staged-arrival wrapper;
- packs, via `introduce_packs`, which needs a dry den in a prey component;
- clutches, via `bury_founding_clutches`, which counts eggs.

Actual counts and failed attempts are recorded.

**Placement:**
1. Pick a viable local component that has the allocated budget.
2. Pick a valid arrival face inside it (route, headroom, water, a usable food patch at the
   site).
3. Reserve resources only for arrivals actually placed.

"Nearest edge" is presentation only. Wrapped seams are not outside edges.

**Scheduler state persists with the save:** the pending roster, readiness history, deadlines
and immigration state.

### R5. Predators [A3]

The harvest budget for a predator lineage is `β × P × y × e ≥ D`, where:

| term | meaning |
|---|---|
| `P` | renewable, eligible prey production, in raw organic units per day |
| `y` | effective carrion yield |
| `e` | the fraction captured and eaten before loss (scavengers, decay) |
| `β` | the allocated harvest share: a tuning knob, starting at 0.1–0.3, not a known bound |
| `D` | predator demand, assimilated |

- **P** is the structural biomass of births plus later structural growth. Reserve transfers
  into offspring are not counted twice. Deaths, and the share available to predators, are
  reported separately.
- **At y = 0.5 and e = 1,** `β` 0.1–0.3 means `P/D ≥ 6.7–20`.
- **Predators also need** a non-depleting prey cohort and enough **standing eligible prey**
  in the prey windows, judged against the participating pack's strength and its reachable
  components.
- **One harvest allocation** is shared across predator lineages.
- **The lanternjaw** also needs a host check: hosts within the hatchlings' reach, and a host
  budget for rider growth.
- **Before any of this, the control defects get fixed** (the empty feasts readout, the
  timeout-heavy chase world). That is package `bellwing`, running now.

### R6. Measures [A9]

- B, R̂, access and sated share stay as diagnostics. They sit next to:
  - cohort size, actual births and maturation;
  - cause-specific deaths;
  - stores and producer stocks over time.
- **Undefined (NaN) is never success.**
- **R̂ is not used for predator admission.** That uses R5's harvestable production.
- **A funded first birth is kept apart from recruitment** sustained by new food.

### Build order

1. **The episode initializer and E0/E1 starts.** The grid needs these first; until they
   exist, it runs on E0, disclosed as such.
2. **Source-level production and withdrawal counters,** and the shared budget (R3).
3. **The staged-arrival scheduler** with the required roster (R4), which also makes E2.
4. **Predator harvest budgets** (R5), after `bellwing` lands.
