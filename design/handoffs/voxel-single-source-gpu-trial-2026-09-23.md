---
status: open
date: 2026-09-23
owner: Fable (organism line), at Wrysk's request
follows: design/handoffs/voxel-training-speed-2026-09-23.md (package G: hand-written CUDA port, passed)
---

# Single-source GPU trial: write a rule once, run it on the CPU and on CUDA

## Why

Package G ported the browser's episode step to hand-written CUDA
(`crates/cubarium-voxel-gpu`, branch `gpu-trial`): 0.94 s for a generation's
landscape batch against 26 s on 7 CPU workers. It is also a **second copy**
of about 40 fauna functions. Senses, ecology and the environment are still
being rewritten, so every change would have to be made twice. Wrysk,
2026-09-23: that coding time counts against the speed-up. Keep CUDA. Water is
the most interesting target, because it is the most expensive.

The question: **can one Rust source serve both the CPU simulation and a CUDA
kernel,** at what speed, and at what cost to writing rules?

## What the desktop profile says (Fable, 2026-09-23)

The run: `cubarium voxel --config config/desktop/terrarium.toml`
(256 × 128 × 48 at 0.125 m), seed 1, the merged `retrain` build 169ae89, 16
threads (taskset), headless, 10 min at speed 4. `perf` sampled 4 min of it.

- **Step:** 17–25 ms/tick steady, peaks 22–76 ms. Water is 94–97 % of it,
  flora about 0.8 ms and animals about 0.2 ms.
- **Inside water (perf, share of all samples):**
  - `exchange_inner_with_masks` 39 %, with `add_pore` 10, `take_pore` 7,
    `offer_up_the_run_mask` 6 and `transfer` 5.5;
  - `drain` 5.5;
  - `CellSet::sorted_into` / `set` 10;
  - `water_table` 3;
  - `take_free` / `add_free` 4.
- **Water is mostly serial.** 66k samples at 199 Hz over 240 s means about
  1.4 of the 16 threads were busy on average.
- **Water in transit is the expensive kind.** About 90k falling cells per
  tick (summed over substeps) persist after the showers: the terrarium's
  stream and falls.

## What to build

Worktree `.claude/worktrees/rules-trial`, branch `rules-trial` off `retrain`
(169ae89, which is main 71c6e4b plus the training line). Merge `gpu-trial`
into it first so the hand-written kernels are there to compare against; that
crate stands alone.

1. **Toolchain (CUDA only).** Find a route by which a Rust function compiles
   for the host and to PTX from the same source. Candidates:
   - Rust-CUDA (`rustc_codegen_nvvm`, `cuda_std`, `cust`);
   - CubeCL's CUDA backend;
   - anything else that meets the criterion.

   Try them on this machine: rustc 1.98, CUDA 13.4 at `/opt/cuda`, RTX 5090 /
   5080, sm_120. Report what failed and why. User-level toolchains (rustup
   nightlies under `~`) are fine. System packages need Wrysk, because sudo
   asks for his password, so ask Fable first.
2. **A rules crate** (`crates/cubarium-rules`, the name is yours): plain,
   allocation-free functions over plain data, no_std where the toolchain needs
   it. Move into it, as the trial:
   - **(a)** the DDA ray walk (`Dda` / `ray_first_hit_cell_in`), step
     resolution (`resolve_motion` and what it needs), and the GRU forward;
   - **(b)** the water **exchange**: the per-cell or per-column rule inside
     `exchange_inner_with_masks`, including `add_pore`, `take_pore`,
     `transfer` and the run mask. Restructure the data layout if the rule
     needs it to be independent per cell or column.
3. **The CPU simulation calls the crate** on this branch: fauna for (a), water
   for (b). This is how we find out what integrating costs. The existing
   suite stays green. Water ledgers close (conservation residual at noise).
4. **The same functions run in CUDA kernels.**
   - For (a), drop them into package G's episode kernel in place of its
     hand-written versions, and compare speed.
   - For (b), write an exchange kernel over the terrarium world.
5. **A CPU-parallel baseline for (b).** The same shared exchange function run
   across 16 threads (rayon over independent columns or regions). The
   restructure may itself unlock the CPU, which is currently at 1.4 of 16.

## Parity is statistical, never bitwise

Wrysk, standing rule: no bit matching, ever. FMA, fast-math and reordering
are allowed and welcome. For water, parity means:
- the ledgers close;
- over seeds, standing volume, pooled cells, lake level and the falling-cell
  count agree between CPU-serial, CPU-parallel and GPU within the seed-to-seed
  spread;
- the picture shows the same ponds and falls (a PNG pair is enough).

For fauna, it means package G's held-out score check: the mean within the
CPU's own 1-ulp chaos spread (about ±1 %).

## Measure

- **Water exchange, ms per tick,** CPU-serial vs CPU-parallel (16 threads) vs
  GPU. Measure on the terrarium desktop world during a shower and at steady
  flow, and include the transfer cost if the GPU version has to copy the grid
  each tick. Say what a GPU-resident water state would need.
- **Fauna (a):** package G's generation batch time with the shared functions
  against its hand-written ones.
- **Development cost, reported plainly:**
  - what the restricted style would not express, and the workarounds;
  - compile times;
  - how debugging a wrong result went;
  - how a rule edit now flows to both targets. Make one small rule edit in
    each family as a demonstration.

## Constraints

- Your own worktree, with `CARGO_TARGET_DIR` inside it.
- **At most 16 threads** on this desktop (`-j 16`). Do not use eidolon.
- **GPU:** check `nvidia-smi` first. ComfyUI, vLLM and Wrysk's own cubarium
  window share the cards. Use the one with room and never kill a process.
- Explicit-path commits only, each ending with the `Co-Authored-By` line you
  were given. Never bare `git stash`.
- **Do not merge anywhere.** Water belongs to Wrysk's terrain and water
  thread: this branch shows a way, and it does not change main's water.
- No tuning of model rates or thresholds. Tests ≤ 200 ticks, no pinned
  hashes.
- Do not edit `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`, or
  the untracked `design/organism-anatomy-2026-09-21.md`.

## Return (≤ 40 lines)

- the toolchain chosen, and what failed;
- which functions went single-source;
- the water and fauna timings in one table;
- the parity statistics;
- the development-cost verdict with its evidence;
- a recommendation: adopt, adopt for water only, or stay with hand-written
  kernels or CPU-only, and what each would take next.
