---
status: open
date: 2026-09-22
owner: Fable (organism line); inserted by Wrysk ahead of P5-C training
---

# Cone speed, and fast hashing on the sim path

## Why

Training (P5-C) runs fauna only — water and plants are frozen — and the
measured cost is the browser's cone: landscape episodes run 10–14k ticks/s per
worker against 73–173k for the browser arena, and the shredder arena is slower
than the browser one because its bystanders are browsers
(`voxel-retrain-2026-09-22.md`, P5-B's measurements). Read from the code
(`crates/cubarium-voxel-fauna/src/senses.rs`, branch `retrain`):

- each browser sample casts 45 rays to 2 m; `ray_first_hit_cell` marches each
  in **fixed quarter-voxel substeps** (32 at 0.25 m, 64 at 0.125 m);
- each step does a material and a water lookup and up to three
  `std::collections::HashMap` lookups (bodies, environment, pools) on SipHash;
- `build_occupancy` rebuilds those maps for every controller stage by walking
  every cell of every profile layer in the window.

Wrysk, 2026-09-22: *anywhere in the game/sim path, optimise hash maps for
speed.*

## What to build

Worktree `cone-speed`, branch `cone-speed` off `retrain`.

1. **Profile first** (`perf` or a timing harness): share of a landscape
   episode's tick spent in occupancy build, ray march, GRU forward, motion,
   the rest — `default` and `small`, 16 bodies. Report it; it is the baseline.
2. **Dense occupancy.** Replace the three maps with flat per-cell storage over
   the window (or the whole world if cheaper — `default` is 221k cells): one
   class byte per cell, pool tops and body ids in side arrays or a small
   sparse list keyed by cell. The readings must be **identical** to the map
   version: a test comparing every ray's `(distance, class, cell)` on a seeded
   landscape with bodies, stands and pools, before switching the march.
3. **Cell-exact traversal.** Replace the fixed substep march with a voxel DDA
   (Amanatides–Woo): visit every cell the ray passes through once, in order,
   to `range`. The first hit's distance is the ray's entry into that cell (for
   water and pools, the entry into the part below the surface/top — solve the
   segment against the plane, don't sample). This changes readings on purpose
   (the substep march can skip a corner-clipped cell); tests: a ray grazing a
   cell corner hits it; a ray along an axis visits exactly the cells under it;
   distances agree with an analytic box intersection; the D3/D4 tests of
   `contract_v2.rs` still pass. Bump the manifest's canonical text so the
   digest records the traversal (it is part of what the policy sees).
4. **Occupancy reuse where it is free.** In static (training) episodes nothing
   but bodies and bites changes: build the environment grid once per episode
   and patch it on a bite or death; bodies are re-indexed per stage. The live
   (dynamic) path may keep the per-stage window build if a cache would be
   stale — your call, with the reason.
5. **Fast hashing on the sim path.** Add `rustc-hash` (FxHash — deterministic,
   no random seed) to the workspace and switch every `HashMap`/`HashSet` in
   the sim crates' non-test code to `FxHashMap`/`FxHashSet`:
   `cubarium-voxel`, `cubarium-voxel-flora`, `cubarium-voxel-fauna`,
   `cubarium-voxel-sim`, `cubarium-core`, `cubarium-search`, and the host's
   per-tick code in `crates/cubarium`. List any you leave and why (e.g. keyed
   by attacker-controlled input — there is none here, but say so). If any code
   iterates a map in a way the result depends on, note it: with std's random
   seed that order was already nondeterministic.
6. **Measure after:** the same profile, landscape and arena ticks/s per worker
   at 16 workers, both lineages, against step 1.

## Tests (first, own commit)

The equality test for item 2 (map vs dense, identical rays); the DDA tests of
item 3; a static episode's patched grid equals a fresh build after a bite and
a death (item 4). ≤ 200 ticks, no pinned hashes.

## Constraints

Only this worktree; `CARGO_TARGET_DIR` inside it; up to 23 cores (a paused
P5-C worker is not running jobs). Explicit-path commits only; end each with
the `Co-Authored-By` line you were given. Sensing semantics other than the
traversal (occlusion rules D3, edge D4, what counts as foliage) do not change.
No tuning. Do not touch the policy files; do not train. Do not edit
`design/handoffs/README.md`, `design/README.md`, `config/tachyon/*`,
`scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`, or the untracked
`design/organism-anatomy-2026-09-21.md`. Full workspace suite (release) before
reporting.

## Return (≤ 30 lines)

Commits; the before/after profile table; ticks/s before/after; choices made
and why; anything left on std hashing and why.
