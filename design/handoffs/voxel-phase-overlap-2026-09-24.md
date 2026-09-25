# Overlap water with plants and animals

Wrysk, 2026-09-24: "we should definitely do the overlapping phase optimization." Splitting
the plant phase further is his call later, once this has results.

## Measured (eidolon, desktop terrarium seed 1, perf, 30 s)

- **Build 86e5ed6** at hour 20 (12k stands, 450 browsers): 44 ms a tick.
  - The main thread did 72 % of all CPU time. The browser mouth scan alone was
    ~47 % (`reachable_layers`, `mouth_foliage_stand`, `layers_of`, `crown_voxels`).
  - Wrysk's hot-cache (8905020f, 3ffb5e2c) already fixed that scan.
- **Build ec7d408** (current main) at minute 390 (11.4k stands, 218 browsers): 14.9 ms,
  against 34.3 ms for the old build at the same minute.
  - Main thread 30 % of the CPU samples, water pool 48 %, task pool 21 %.
  - The main thread sits at ~75 % busy: the tick is mostly one-thread work followed by
    a parallel burst.
  - Main-thread hot spots: flora `step`, `drink_with`, the serial part of
    `light_per_stand`, `LayerIter`, `Vec<StandLayer>` collects, `root_box_dims_into`,
    `sky_at`, fauna `build_occupancy`.
- The schedule is `.chain()`ed on the single-threaded executor
  (`crates/cubarium-voxel-sim/src/lib.rs:244`), so water, flora and fauna never run at
  the same time.

## The change

Run **Water(t) ‖ (Flora(t) → Fauna(t))**, then a barrier.

- **Plants and animals read the world as it stood after the previous tick's water.**
  That is a one-tick (50 ms) lag, which no one can see. It is the only rule change
  allowed.
- **Everything flora writes to the world is buffered and applied at the barrier.**
  List every write (drink, transpiration into the atmosphere, anything else you find).
  - A drink takes `min(want, what the cell holds now)`.
  - The stand is debited any shortfall at the barrier, so the water ledger stays exact.
- **Fauna should only read the world** (`sys_fauna` takes `Res<VoxelWorld>`). Confirm
  that.
- **Preferred route: let bevy do it.** Give the bio chain its own resource for the world
  it reads (a snapshot), and a resource for the pending writes. Water systems then touch
  only `VoxelWorld`, and bio systems only `(snapshot, Flora, Fauna, pending)`. Switch
  the tick schedule to the multi-threaded executor and order both chains before a
  barrier system that applies the writes and refreshes the snapshot.
  - The substep schedule and the static, frozen and arena schedules keep their current
    behaviour. Training uses the static arena.
- **Snapshot cost:** find the smallest read set flora and fauna actually need through
  `VoxelView`. Refresh it in parallel, or only where it changed. Budget ≤ 1 ms at the
  terrarium; if a plain copy is over that, go incremental.
- **Thread pools:** water has its own pool (`water-N`) and bio uses `ComputeTaskPool`.
  Running both at once can put 2N busy threads on N cores. Measure the options (both at
  N, or split N between them), pick one, and report the numbers.
- **A switch:** `SimConfig` gets `overlap` (default on). Off is the old chained tick, so
  runs can be compared.

## Not in this package

No flora or fauna rule, rate or threshold moves, and no parallel work inside flora beyond
what exists (that is option 1, Wrysk's call later). No snapshot schema change: saves
happen at tick boundaries, where nothing is pending. Determinism is not required
(no-bit-identical); the ledger must still close.

## Tests (write these first; tiny, a few ticks)

1. Water residual ≤ 1e-9 over ~200 overlapped ticks, with a shower and plants drinking.
2. A plant drinking from a cell that water emptied in the same tick gets only what is
   left, and the shortfall comes off its stand. The ledger is exact.
3. With `overlap` off, the tick is the old chained tick, with the same phase order.
4. Fauna in tick t sees water depth from the end of tick t−1.

## Acceptance and return

- **Speed:** the terrarium census at seed 1 on eidolon.
  - Build with `-C target-cpu=znver5`, `CARGO_TARGET_DIR=target/znver5`; ship the
    binary to `~/cubarium-runs/<dir>/`.
  - Pin by physical core: CPU n and n+6 are SMT siblings.
  - Compare `tick_ms` at matched minutes against
    `~/cubarium-runs/terr24-ec7d408/s1.csv`, running long enough to pass minute ~400.
  - Target ≥ 1.3× by then.
  - Also report per-phase times with the `profile` feature, before and after.
- **Behaviour:** default-preset 10 min autopsies (seeds 1–3), inside the seed spread,
  the way 3ffb5e2c checked.
- **Return:** ≤ 40 lines — commits, the numbers, the thread-pool choice and why, and
  anything left serial with the reason. The commit message is the report.
