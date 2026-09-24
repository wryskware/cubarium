---
status: open
date: 2026-09-23
owner: Fable (organism line), under Wrysk's delegation of the organism decisions
follows: design/handoffs/voxel-retrain-c-2026-09-22.md (P5-C, not shipped: drownings up)
---

# Training speed — groundwork for the next retrain

## Why

P5-C's ES took 4.5 h (browser, 7 workers) and 12.3 h (shredder, 16 workers)
for 512 updates each. Held-out scores, updates 0 / 32 / 96 / best: browser
0.80 / 1.02 / 1.04 / 1.165 (update 512); shredder 0.46 / 1.37 / 1.70 / 1.80
(update 288). Most of the gain arrives in the first ~100 updates. Wrysk,
2026-09-23: 6–12 h per iteration is not a sustainable pace; use the GPUs if
possible, and a second machine.

The next retrain waits for packages still landing on main: organism sizes at
the ladder (package L, merged), new plants (N), plant viability (W/F/G/S). L
moves the body anchors, so the manifest digest moves, and the strict loader
refuses today's centres as a warm start. This package builds the tools; it
does **not** train.

## Machines and limits (Wrysk, 2026-09-23)

- **This desktop** (Ryzen 9 9950X3D, 32 threads; RTX 5090 32 GB + RTX 5080
  16 GB, Vulkan 1.4, no CUDA toolkit): at most **50 %** of the CPU — cargo and
  test runs at `-j 16` / 16 threads. Other packages are running here too.
- **eidolon.local** (Ryzen 5 9600X, 12 threads, 60 GB, rustc 1.98.1 like this
  machine, ssh without a password): **all of it**. Write only under
  `~/cubarium-train/` there; touch nothing else on that machine.
- **No training or gate runs.** Smoke runs only: at most 5 min wall each, tiny
  settings (a couple of pairs, updates and fixtures). Wrysk gives the go for
  the next real run.

## Package S — turnaround and a second machine

Worktree `.claude/worktrees/train-speed`, branch `train-speed` off `retrain`
(bb0cf40). Files: `crates/cubarium-search/src/es/voxel/*`, the search CLI,
the autopsy example.

1. **Transfer start.** `voxel-train --transfer-from <centre.json>`: accept a
   centre whose vector length equals the founder manifest's parameter count
   even when its digest differs, and write the source file and digest into the
   run's provenance. Refuse a length mismatch by name. `--init-center`, the
   live loader and the shipped-centre checks stay strict: this is a training
   start only, and the gate judges what comes out. The next run then compares
   update 0 on held-out for three starts (transfer, clone, both) and keeps the
   best. That comparison happens at run time, not in this package.
2. **Plateau stop.** `--plateau N,g` stops when the held-out best has not risen
   by a relative `g` for `N` checkpoints; the P5-C fall rule (C5) stays.
   Default for `--p5` runs: 3 checkpoints, 0.02. `--heldout-every K` (default
   32) so a fine-tune can check every 16.
3. **Remote workers.** `voxel-train ... --remote eidolon.local:12` starts
   `ssh eidolon.local ~/cubarium-train/cubarium-search voxel-eval-worker
   --threads 12` and speaks to it over the ssh pipe. No open ports, and ssh
   handles authentication. Requirements:
   - The unit of work is one antithetic pair on one fixture, and **both signs
     run on the same machine**, so a machine's float quirks cancel in the pair.
     Jobs go to whichever machine is idle. Held-out evaluation stays local.
   - Both machines evaluate **the same worlds**. Either ship the frozen
     fixtures or rebuild them on the worker from the seed list and compare a
     per-fixture summary (founders, accepted components, recorded production
     totals). A mismatch refuses that remote loudly. Your choice; say why.
   - The worker's binary must be the coordinator's build: compare the git
     hash and build flags at connect. Build it for Zen 5 (both CPUs are Zen 5;
     `-C target-cpu=znver5` or native on each, your call), and ship or build
     it under `~/cubarium-train/`.
   - A remote that dies or stalls mid-generation has its outstanding jobs
     re-queued locally. The run goes on, with one loud line.
4. **Drowning record.** `voxel_founder_autopsy` prints a line per drowning:
   - minute, lineage, born in the run or founder;
   - the depth read and how many cells it counted;
   - the bottom cell's fill;
   - whether it was raining that tick;
   - the depth read 100 ticks earlier at that face.

   The next gate then splits "rain column counted as depth" from rising water
   without a separate probe. If `VoxelView::standing_depth_m` (package N, on
   main) is reachable after the main merge, add it as a column; it is not on
   `retrain` yet, so don't port it.

**Tests (first, own commit, ≤ 200 ticks, no pinned hashes):**
- Transfer accepts a same-length, different-digest centre and records its
  provenance; it refuses a wrong-length one; `--init-center` still refuses a
  digest mismatch.
- The plateau rule stops on a synthetic held-out series and does not stop on a
  rising one.
- A generation evaluated with a loopback remote gives the same update as a
  local-only generation, within float tolerance. The loopback is the worker
  started locally through `sh -c`, not ssh.
- A remote killed mid-generation still yields a complete generation.
- The drowning line appears for a hand-built drowning.

Then one real smoke through ssh to eidolon: 2 pairs, 2 updates, 4 landscapes.
Report generation time local-only against local + eidolon at the same
settings, and extrapolate to P5-C's per-generation cost (browser 28 s at 7
workers, shredder 87 s at 16).

## Package G — GPU trial (browser, frozen landscapes)

Worktree `.claude/worktrees/gpu-trial`, branch `gpu-trial` off `retrain`
(bb0cf40). A new crate, `crates/cubarium-voxel-gpu`, plus an example or
search subcommand for the benchmark. It does **not** wire into the trainer and
does not change any existing crate's behaviour.

The question: how many browser body-ticks per second can one GPU step on
frozen landscapes, and do its episodes agree with the CPU's?

- **API.** wgpu over Vulkan is the default: it works on both cards with no new
  system installs. CUDA would need a toolkit install, so ask Fable before
  going that way. Keeping the rules readable and in one place matters more
  than the last 20 %. If a Rust-source kernel route (CubeCL, rust-gpu) lets
  the kernel share rule code or constants with `cubarium-voxel-fauna`, weigh
  it and say what you chose and why.
- **GPU etiquette.** Check `nvidia-smi` first. Other programs (ComfyUI) may
  hold memory: use whichever card has room, never kill a process, and keep
  within the free memory.
- **G0 — profile.** Take the CPU profile of a browser landscape episode tick
  on this branch as the baseline: occupancy, cone, GRU, contact, motion,
  bites and metabolism, the rest. The cone-speed package has a harness.
- **G1 — senses + brain.** Batch N bodies × fixtures: the dense occupancy as a
  read-only 3D texture or buffer, the 45-ray DDA cone and the contact probes,
  and the GRU32 forward, fed from **recorded CPU body states**.
  - Parity: every ray's class and cell equal, distance within 1e-4 m, every
    observation channel within 1e-4, over recorded ticks of the 24 held-out
    fixtures.
  - Measure body-ticks/s against the CPU's same share.
  - If this is under 5× the CPU's rate for the same work at 16 threads, stop
    and report.
- **G2 — whole episodes.** Motion (the step rule, climb, wade refusal, the z
  wall), bites against per-episode food state, metabolism and the score
  ((intake − motor)/body_ref + 0.25·survived), births off, as in training.
  Each episode owns its food state.
  - Parity: run the P5-C browser candidate
    (`runs/p5c/es-browser/centers/upd512-heldout.json` in the retrain
    worktree) and the heuristic-free stationary policy on the 24 held-out
    fixtures, CPU against GPU.
  - Report per-fixture scores side by side, the mean held-out score
    difference, and every fixture differing by more than 5 %, with the first
    tick where trajectories part and why.
  - Trajectories may drift (float order); the scores must not.
- **Measure:** episodes/s and body-ticks/s at the batch size one ES
  generation would use (65 variants × 16 landscapes × ~8 bodies), and the
  projected seconds per generation against 28 s on 7 CPU workers.

**Tests (first, own commit, ≤ 200 ticks):**
- Ray parity on a hand-built grid: an axis ray, a corner graze, water below
  the surface, the z-edge wall.
- Observation parity on one seeded landscape for 50 ticks from recorded
  states.
- A GRU forward matches `cubarium-core`'s on random weights.
- A one-episode, 200-tick run's score is within 1 % of the CPU's.

Package L on main changed the mouth ("probes its whole region") and adult
sizes. Port `retrain`'s rules as they are, and list in the report which
functions you ported, so the port can follow L after the merge.

## Constraints (both)

- Your own worktree; `CARGO_TARGET_DIR` inside it; the CPU limits above.
- Explicit-path commits only, each ending with the `Co-Authored-By` line you
  were given.
- No tuning of model rates or thresholds. Always fresh worlds.
- Do not merge to main or to `retrain`.
- Do not edit `design/handoffs/README.md`, `design/README.md`,
  `config/tachyon/*`, `scripts/tachyon-*`, `docs/tachyon.md`, `art/gen/*`, or
  the untracked `design/organism-anatomy-2026-09-21.md`.
- No full-suite requirement for G (a new crate): its own tests, plus
  `cargo check --workspace`. S: the full workspace suite (release, 16
  threads) before reporting.

## Return (≤ 40 lines each)

Commits; choices made and why; the measurements with how they were taken;
parity results with the worst cases; what you would do next and what it would
cost.
