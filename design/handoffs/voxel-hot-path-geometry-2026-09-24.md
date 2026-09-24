---
status: open
date: 2026-09-24
owner: Fable (organism line); Wrysk said go 2026-09-24
follows: design/handoffs/voxel-cache-and-pinning-2026-09-24.md (merged d252993)
---

# Hot-path geometry: stop recomputing what did not change

## Why

The cache study found training compute-bound, not cache-bound: L3 misses
are about 0.01 per 1k instructions. A browser landscape tick still spends:

| cost | share of the tick | why |
|---|---|---|
| `crown_voxels` | 9.5 % | every stand's crown geometry, recomputed for every stand, mouth and tick, although wood changes slowly (never, in a frozen training world) |
| `is_support` (in motion) | 12.8 % | the standable-face test, walked cell by cell per candidate step |
| `reachable_layers` | 10 % | the mouth's layer search, recomputed per bite attempt |
| the detritus field (`DetritusField`) | about 6 % | hash maps and sorts |

The same functions run in the live world and in the gate's autopsies, so all
three uses gain.

## What to build

Worktree `.claude/worktrees/hot-cache`, branch `hot-cache` off main
(d252993).

1. **Profile first.** Take a flat `perf` profile of a browser and a shredder
   landscape episode (`voxel-landscapes --arms` or the counters,
   `CUBARIUM_COUNTERS=1`) and of the headless terrarium live step. That is
   the baseline table, by function.
2. **Crown geometry, cached per stand.** Crown voxels, the layer bands and
   anything derived only from species, wood and voxel size is computed when a
   stand is seeded or its wood changes, and read afterwards. The invalidation
   must be exact in the live world: growth, dieback, the vaulttree fall, death
   and new seedlings. Put the cache where the flora owns the truth (on the
   stand, or in a dense side array that flora keeps current), not in the
   fauna.
3. **Standable faces.** Replace the per-candidate `is_support` walk with a
   lookup kept current:
   - a dense bit per cell, or per column the list of support faces;
   - or whatever the profile shows the walk is actually spending on.

   Terrain edits (`terrain_version`) invalidate it. If support depends on
   water or plants, and not only on material, account for that and say how.
   Frozen episodes build it once.
4. **The mouth's reach.** `reachable_layers` / `reachable_layer_stock` read
   the cached geometry and not a recomputation. Only stock changes on a bite.
5. **The detritus field in dense arrays,** keyed by cell or column index; no
   hash maps or sorts in its per-tick update.
6. **Anything else the profile names** above 3 % that is the same kind of
   thing: frozen or slow-changing data recomputed per tick. Leave the water
   scheduling alone (its fork-join is a separate question).

## Correctness

These are execution-only changes: the rules, rates and thresholds don't
move.

Tests, written first in their own commit, ≤ 200 ticks, no pinned hashes:
- **The cache equals a fresh computation** for every stand after growth, a
  bite, a death, a fall and a new seedling.
- **The support lookup equals the direct test** for every cell after a
  terrain edit.
- **The dense detritus field equals the map version's readings.**
- **Statistics hold over seeds 1–3:** alive counts, bites and pooled water
  within the seed spread, from a short autopsy (`threads=`, a few minutes)
  before and after.

## Measure

Measure before and after:
- ticks/s per worker for browser and shredder landscapes at 16 workers;
- the landscape-tick profile shares;
- the live terrarium step in ms/tick (headless, `--seed 1 --sink png --fps 1
  --every 1000000 --speed 4`).

## Constraints

- **CPUs:** this desktop's **CCD1** only (`taskset -c 8-15,24-31`, cargo
  `-j 16`). CCD0 belongs to Wrysk and the live app. eidolon.local may be used
  in full, writing only under `~/cubarium-train/`.
- **No training or gate runs,** only benchmarks and short smokes (≤ 5 min).
- Use FxHash on sim paths and dense arrays for cell keys.
- **Git:** your own worktree, with `CARGO_TARGET_DIR` inside it.
  Explicit-path commits only, each ending with the `Co-Authored-By` and
  `Claude-Session` lines you were given. Never bare `git stash`. Do not merge
  to main; if main moves, merge it into your branch before reporting.
- **Do not edit** `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`, or
  the untracked `design/organism-anatomy-2026-09-21.md`.
- **Before reporting,** run the full workspace suite (release, on CCD1).

## Return (≤ 40 lines)

- commits;
- the before/after profile table;
- ticks/s per worker before/after for both lineages;
- the live step before/after;
- each cache's invalidation rule and its test;
- the statistics check;
- what is left.
