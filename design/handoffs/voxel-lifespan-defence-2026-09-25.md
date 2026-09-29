---
status: open
date: 2026-09-25
owner: Fable (organism line); Wrysk said go 2026-09-25 ("wtf there was no senescence!? yeah do the fixes")
follows: voxel-ecology-search-2026-09-24.md (branch live-train)
---

# Two model fixes, then re-search and retrain

The ecology search found no steady band. Animals die only of starvation and drowning,
so a world grows until food per head runs out. The browsers park because a crown feeds
a parked browser indefinitely. Two live-rule changes, allowed for these two fixes only:

## A. Animal lifespan: a new departure `Aged`

- Each body's lifespan is drawn at birth: its lineage's median × a spread of about ±20 %.
  The implementer chooses the distribution and says which.
- Founders start at staggered ages, so they don't die off as one cohort.
- The median is a per-lineage config field, 4 h of simulated time by default for both
  current lineages. Say why if the data argues for another value.
- An aged death leaves carrion exactly as the other deaths do. The ledger and the census
  count it by cause.
- Lineage-generic: a new lineage sets its median. A schema bump is fine (always fresh,
  never migrate).

## B. Plants fight back: an induced grazing defence

- Each plant carries a defence level that rises with every bite taken from it.
- The bite shrinks with the defence: less foliage is removed, and the intake falls in
  proportion.
- The level decays once grazing stops.
- Targets for the defaults, tuned from a short measurement (state the numbers):
  - continuous grazing of one crown cuts intake to about 25 % or less within about
    15 min;
  - recovery has a half-life of about 1–2 h;
  - an ungrazed neighbour gives full bites.
- Scope: the crowns browsers eat. Strength and half-life are per-species config fields,
  with the same defaults for every species. The defence is also a search parameter.
- The refuge and the reflush line are unchanged.
- **No sense input is added or changed.** Animals feel the defence only through their
  own intake, so round 2's centres must still load under the contract.

## Tests (≤ 200 ticks each, short test configs, no pinned hashes)

- A body past its lifespan departs as `Aged` and leaves carrion, and the ledger balances.
- A newborn gets its own lifespan. The founders' ages at death are spread out, not
  simultaneous.
- Repeated bites on one plant give strictly decreasing intake. A fresh neighbour gives a
  full bite. The defence decays toward zero after grazing stops.
- Mass and energy conservation hold with the defence on.
- Round 2's centres still load.

## C. Re-search, then retrain

1. **Ecology search** (CMA-ES, 3 h runs projected onto the 6 h band).
   - Add the lifespan median, the defence strength and the defence half-life as
     parameters.
   - Hard constraint: every lineage has births in the second half, which closes the
     stop-breeding cheat.
   - Persistence counts only the species the default world keeps on that seed.
   - At most 3 h of wall time.
2. **Held-out check** as before: seeds 1–4, 6 h, tuned against the new defaults, round
   2's brains. Report population curves, moving share, births and deaths by cause
   (aged included), and plants per species.
3. **Brain retrain** in the tuned world:
   - round 2's centres, at most 2 h, 8 pairs per lineage;
   - first a short σ = 0 control, to show the signal exceeds the placement noise;
   - then the gate as before, the trial's centres against round 2's, in the tuned world.

Also report the live-default picture: a 6 h census with the new defaults (lifespan and
defence on), untuned. That is what the live app would do if merged.

**The question:** do browsers move now (moving share ≥ 0.30), and does the population
hold a steady band, with births balancing deaths?

## Constraints

- CCD1 under `nice -n 10`.
- Explicit-path commits with the two trailer lines. No merge to main.
- The full suite before reporting.
- Do not end a turn while anything is running.
- Return ≤ 40 lines.
