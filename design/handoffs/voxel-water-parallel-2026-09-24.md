---
status: open
date: 2026-09-24
owner: Fable (organism line), run here at Wrysk's request; water belongs to the terrain/water thread, and Fable coordinates the merge
follows: design/handoffs/voxel-single-source-gpu-trial-2026-09-23.md (branch `rules-trial`, faabb5b)
---

# Parallel water on the CPU

## Why

The desktop profile (terrarium, 256 × 128 × 48 at 0.125 m, 16 threads) puts
water at 94–97 % of a tick, with about 1.4 of the 16 threads busy. The
single-source trial rewrote the water exchange per column and ran the same
rules on a rayon pool. Fable re-ran seed 2 at 16 threads:

| ms/tick | serial | rayon ×16 |
|---|---|---|
| exchange, steady | 10.8 | 2.3 |
| exchange, shower | 13.8 | 3.0 |
| water tick, steady | 13.8 | 5.2 |
| water tick, shower | 22.7 | 11.7 |

Standing volume, pooled cells, falling cells and lake level were identical
across drivers to 4 decimals, and the ledgers closed. In a shower, most of the
remaining ~8.7 ms is phases that are still serial: drain, fall, infiltrate,
the water table and the wet-set bookkeeping (`CellSet::sorted_into`/`set`).

## What to build

Worktree `.claude/worktrees/water-par`, branch `water-par` off main
(a2cee21, which already has retrain and mobility merged).

**W1: bring the exchange over.** From `rules-trial`, take only the water
part:
- the `cubarium-rules` crate's `water` module and the `math` shim it uses;
- the per-column exchange in `crates/cubarium-voxel/src/water.rs`;
- the per-column row masks in `sparse.rs`;
- the `Serial` and rayon `Parallel` drivers behind `ExchangeBackend`.

Leave these behind on `rules-trial`:
- the fauna rules (ray, motion, GRU);
- the CUDA driver, the PTX crate and the nightly build.

Keep `cubarium-rules` no_std-compatible so the GPU driver can come back
later.

- **One exchange, not two.** After a one-time statistical check on this
  branch, remove the old mask exchange path. Tests check ledgers and
  statistics, never bits.
- **The live schedule runs `Parallel` on the sim's own pool.** Take
  `SimConfig::threads`, the process's compute pool. `threads = 1` runs
  serial. A tool that runs many sims side by side (training workers, gate
  runs) must not oversubscribe the machine: say how the pool size reaches the
  water, and that one thread gives the serial path.
- Worlds over 128 rows keep the dense path they have now. No preset is that
  tall.

**W2: the phases that are still serial.** Profile the water tick after W1
(`perf` or the bench's timers). Then move the phases that show, in order of
cost, to per-column rules functions run on the same pool:
- drain;
- fall;
- infiltrate;
- the water table;
- the wet-set and drainable-set bookkeeping;
- rain and evaporation, if they show.

Where a phase is truly sequential, for example flow that must be ordered down
a column, parallelise across columns and keep each column's order. Stop when
the remaining serial share is small or the next phase would need a new
algorithm. Report that as the end point, with its profile.

## Parity (statistical, never bitwise)

- The ledgers close (conservation residual at noise).
- Over seeds 1–3 on the terrarium desktop world and on `small`, `default` and
  `wide`, serial against parallel: standing volume, pooled cells, falling
  cells and lake level agree within the seed-to-seed spread after 1,200 ticks
  through a shower.
- A PNG pair, serial and parallel, shows the same ponds and falls.
- The existing water tests stay green. Change a test only where it pinned an
  implementation detail the brief replaces, and say which.

## Measure

- **Water tick ms, serial against 16 threads,** steady and in a shower:
  terrarium desktop, seeds 1–3; W1 alone, then W1 + W2.
- **The whole live step:** a 10-minute headless run of `cubarium voxel
  --config config/desktop/terrarium.toml --seed 1 --sink png --fps 1 --every
  1000000 --speed 4` under `taskset -c 0-7,16-23`. Read the app's
  `tick cost` lines against Fable's baseline: step 17–25 ms/tick, water
  94–97 %.
- **The panel, by proxy:** the `small` preset at 7 threads. The Tachyon runs
  `threads = available − 1` = 7 on A55/A78 cores. A small world must not get
  slower with the pool; if it does, add a size threshold under which water
  runs serial, and state it.

## Constraints

- Your own worktree, with `CARGO_TARGET_DIR` inside it.
- At most 16 threads on this desktop (`-j 16`). Do not use eidolon.
- Explicit-path commits only, each ending with the `Co-Authored-By` and
  `Claude-Session` lines you were given. Tests go in their own first commit
  (≤ 200 ticks, no pinned hashes). Never bare `git stash`.
- **Do not merge to main.** Fable merges after reviewing, and checks main for
  the water thread's own commits first. If main moves while you work, merge it
  into your branch before reporting.
- No tuning of model rates or thresholds.
- Use FxHashMap/FxHashSet on sim paths, and dense arrays for cell keys.
- Do not edit `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`, or
  the untracked `design/organism-anatomy-2026-09-21.md`.
- Full workspace suite (release, 16 threads) before reporting.

## Return (≤ 40 lines)

- commits;
- what came over from `rules-trial` and what stayed behind;
- how the pool size reaches the water;
- the timing tables (water tick for W1 and W1 + W2, the live step, `small`
  at 7 threads);
- the parity statistics;
- which serial phases remain, and why you stopped there.
