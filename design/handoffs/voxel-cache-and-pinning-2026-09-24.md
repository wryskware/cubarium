---
status: open
date: 2026-09-24
owner: Fable (organism line), at Wrysk's request
---

# Cache-aware training and simulation on this desktop

## Why

Wrysk, 2026-09-24: consider how to pin threads on this machine to make the
best use of cache during training, and make any cache optimisations the code
can take more generally. Tailoring to this PC is fine: training code is not
shipped to other machines.

## The machines

- **Desktop:** Ryzen 9 9950X3D, two chiplets (CCDs), each core with its own
  L1/L2.
  - **CCD0** = CPUs **0–7 and their SMT siblings 16–23**, sharing a **96 MB
    L3** (the V-cache chiplet).
  - **CCD1** = CPUs **8–15 and 24–31**, sharing **32 MB**.
  - Wrysk's current limit for agent work is 50 % of this machine: in practice,
    **CCD0 only** (`taskset -c 0-7,16-23`), which leaves CCD1 to him. Design
    for the full machine too, for when the limit lifts.
- **eidolon.local:** Ryzen 5 9600X, one CCD, **cores 0–5 and SMT siblings
  6–11**, 32 MB L3. All of it may be used. Write only under
  `~/cubarium-train/` (`scripts/voxel-remote-ship.sh` ships the znver5
  binary there).

## The training hot path, as it stands

`cubarium-search voxel-train --p5`:
- ES workers are threads in one process. Each runs whole episodes on one
  thread (`SimConfig { threads: 1 }`).
- A generation is 65 variants × 32 fixtures: 16 arenas, plus 16 frozen
  landscapes drawn from 96.
- The frozen landscape pool is built at startup and is gigabytes in memory.
  Remote workers on eidolon rebuild it (package S, `es/voxel/remote.rs`).
- Held-out checks run every 32 updates.
- **CPU profile of a browser landscape tick** (package G's G0, one thread):
  - cone rays 19 %;
  - the bystander shredders' own senses 22 %;
  - motion 13 %;
  - taste 10 %;
  - GRU 8 %;
  - bites 7 %;
  - controller/observation 7 %;
  - driver bookkeeping 7 %.

## What to do

Worktree `.claude/worktrees/cache-speed`, branch `cache-speed` off main.

**A. Measure first, with hardware counters.** `perf stat` works for your own
processes here (`perf_event_paranoid` 2). Take cycles, instructions, IPC,
L1d, L2 and L3 misses (AMD's own events where the generic ones are vague),
and ticks/s per worker and in total. Use the P5 landscape set at P5-C's
settings, on `voxel-landscapes` and a 2-update `voxel-train` smoke, for both
lineages. Arms on the desktop:
- unpinned, 16 workers;
- 16 pinned to CCD0, one per physical core, then the siblings;
- 8 pinned to CCD0's physical cores only (no SMT);
- for the full machine later, 32 across both CCDs, and 16 on CCD1 alone (the
  32 MB chiplet, to price the V-cache).

On eidolon: 6 against 12, pinned. Repeat each arm (at least 3), and report
the spread: the machine is shared.

**B. Pinning in the trainer.** Add `--pin auto|<cpu list>` to `voxel-train`,
`voxel-eval-worker` and `voxel-holdout`:
- Worker `i` takes one CPU.
- `auto` reads the topology from sysfs: L3 groups largest first, physical
  cores before SMT siblings, within the current affinity mask. `taskset` or a
  cgroup limit is respected.
- The coordinator passes `--pin` to its remote workers.
- The default is whatever A shows to be best on this machine; say what it is.

**C. Cache locality in the training path, guided by A's counters.**
- **Schedule for reuse.** Right now any idle worker takes any (pair, fixture)
  unit. Consider giving each worker runs of units on the **same fixture**, so
  that world stays hot in its core's L2 and the chiplet's L3; or ordering
  units fixture-major. Measure it. Keep package S's rule that both signs of a
  pair run on one machine.
- **Sharing, not copying.** Check whether the frozen world, the occupancy
  grid and the replay tracks are shared read-only between workers or copied
  per episode. Share what is read-only. Keep the per-episode mutable part
  (food eaten, bodies) small and contiguous.
- **Layout and allocation in the per-tick loop:**
  - no allocation per tick; reuse scratch per worker;
  - structure-of-arrays for bodies where the hot loop touches few fields;
  - compact cell encodings where the hot loop reads them (the dense
    occupancy is one byte a cell already);
  - a tiled or Morton cell order, only if the counters say the grid walk
    misses.
- Change what A's counters point at, and nothing speculatively.

**D. The live simulation, the same way, after C.** Take `perf stat` on the
headless terrarium run: `cubarium voxel --config
config/desktop/terrarium.toml --seed 1 --sink png --fps 1 --every 1000000
--speed 4`, pinned to CCD0. The step is now about 4 ms/tick, mostly water.
- The water exchange scales 9.7 → 1.9 ms from 1 to 16 threads. Find whether
  memory bandwidth, the pool's scheduling or false sharing limits it, and fix
  what is cheap.
- Pin the live loop's pool to CCD0 on this machine if that pays. Leave the
  Tachyon's own pinning (`--pin-loop`, `voxel/placement.rs`) alone.

## Rules

- **Execution only.** No rule, rate or threshold changes; the ecology is
  untouched.
- Parity is statistical, never bitwise. Reordering is fine, and so are
  FMA-enabled builds (training may use `-C target-cpu=znver5`/native).
- No training or gate runs: benchmarks and smokes only (≤ 5 min each).
- Tests go in their own first commit (≤ 200 ticks, no pinned hashes). The
  pinning parser is tested on a synthetic topology, not this machine's.
- Your own worktree, with `CARGO_TARGET_DIR` inside it. Explicit-path commits,
  each ending with the `Co-Authored-By` and `Claude-Session` lines you were
  given. Never bare `git stash`. Do not merge to main.
- **Do not edit** `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`,
  or the untracked `design/organism-anatomy-2026-09-21.md`.
- Use FxHash on sim paths.
- The full workspace suite (release, CCD0) before reporting.

## Return (≤ 40 lines)

- commits;
- A's table (arms × ticks/s per worker and total, IPC, L2/L3 miss rates, with
  spread);
- the pinning default and why;
- each C and D change, with its measured effect;
- what the counters say is left, and what it would take.
