---
status: open
date: 2026-09-24
owner: Fable (organism line); Wrysk said go 2026-09-24
follows: voxel-live-world-training-2026-09-24.md (branch live-train)
---

# Ecology search: bounded worlds with moving browsers

The live-world training run (`runs/lt1` on branch `live-train`) showed that parking
survives live training. The worlds also boom without limit: every small still-water
world passed 300 animals within about 4.5 simulated hours. Per that brief, the lever is
the ecology. This package is the ecology half: a world-level search over the ecology
parameter vector.

**Goal:** find ecology settings under which small, still-water worlds stay bounded and
the browsers move.

Branch `live-train`, same worktree. Commit this brief first, then the tests, then the code.

## Search

- **Parameters:** the five routed in `es::voxel::live::ecology`:
  - `flora.foliage_rate`
  - `flora.senescence`
  - `flora.decomposition`
  - `founder.yield_by_food`
  - `founder.reproduction.surplus_floor`

  All five are searched in log space over 0.25×–4× default, unless the model breaks
  sooner. Add a parameter only if none of these can move the population, and say why.
- **Candidates:** each candidate is a parameter vector scored on **freshly founded**
  worlds, not persistent ones. Population takes hours to respond.
  - Use common random numbers: every candidate in a generation runs on the same seeds.
- **Method:** CMA-ES (5 dimensions, about 16 evaluations per generation) or antithetic
  ES. Give the reason for the choice.
- **Horizon:** 6 simulated hours if the wall budget allows at least 8 generations.
  Otherwise shorter, with the score taken from the trend. Measure first.
- **Brains:** held at round 2's centres with σ = 0, so the ecology result is readable.
  Keep the unified mode (brain ES running at the same time) as a flag, but do not use
  it in this run.
- **Fitness:** a penalty. It is zero inside the band and grows with the normalised
  distance outside it.
  - **Boundedness:** animals per small world stay within 30–80 over the second half of
    the horizon, and the peak never exceeds 120.
  - **Liveliness:** browser moving share of at least 0.30.
  - **Persistence:** a hard constraint with a large penalty. Both animal lineages and
    every plant species are present at the end, so the search cannot bound the
    population by starving everything.
  - Log every readout factor per candidate, even though only these three score.

## Tests (their own commit, each ≤ 200 ticks, no pinned hashes)

- Each routed parameter changes its process in the expected direction.
- The penalty is zero inside the band and monotonic outside it.
- On a toy objective, the search moves toward the known optimum.
- The live app's defaults are unchanged.

## Run

- At most 3 h of wall time on CCD1 under `nice -n 10`. Another session's census may be
  on CCD1; leave it alone.
- Then a check: 6 h censuses on held-out seeds 1–4, the tuned vector against the
  defaults, with round 2's brains. Report:
  - the population curve;
  - moving share;
  - births, and deaths by cause;
  - plant stands per species.

## Constraints

- The live defaults do not change. The tuned vector goes in a training-only config file.
- Coarse water at k = 4 is fine.
- Everything else from the live-training brief still holds:
  - explicit-path commits with the two trailer lines;
  - no merge to main;
  - the full suite before reporting;
  - no windows.

## Return (≤ 40 lines)

- commits;
- the search method, horizon and generations, with reasons;
- the best vector against the defaults, with each parameter's multiplier;
- the penalty curve;
- the held-out check;
- which parameters mattered;
- anything that looks like the model can't reach the band.
