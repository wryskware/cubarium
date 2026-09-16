---
design_status: exploration
last_reviewed: 2026-09-16
---

# GS-1b (Opus, high): the live ring world on the GPU

Stage B of the Vulkan renderer: the real world instead of the synthetic
scene. Read, in order: `design/7_Research/gs1-vulkan-renderer-2026-09-16.md`
(your Stage A), `design/7_Research/flat-world-fw4-2026-09-16.md` (`Output`,
`WorldShape`, `FrameSink`, the CLI, the runner's loop), `design/7_Research/
flat-world-fw2-2026-09-16.md` (`RenderView` fields), `crates/cubarium/src/
art_present/mod.rs` via `graft skeleton` (how the CPU presenter turns a
`RenderView` into stamps: slot tables, growth stages, wind, columns, bodies,
hunters), `crates/cubarium/src/lanternjaw/**` skeletons (the rig parts), and
the plan's viewing-session note in `design/handoffs/tachyon-screen-plan-2026-09-16.md`.
No nested agents.

## Objective

`cubarium run --fresh --topology ring:640x360 --world-scale 2 --sink gpu`
on the Tachyon shows the live world on the panel at 60 fps through the
daemon's socket, as an unprivileged user; on the desktop the same flag opens
the window. The CPU presenter stays the reference: the GPU picture of a
frame should match the CPU ring capture of the same `RenderView` closely
(measured, not asserted identical).

## Where and what you own

Worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`,
branch `tachyon-screen`. You own `crates/cubarium-gpu/**`, a new
`crates/cubarium/src/sink/gpu.rs` (+ its `mod` line and the `RunSinkArg`/
`SinkArg` variant in `cli.rs`, the `open_sink` arm in `runner/mod.rs`, and
the `Cargo.toml` dependency), nothing else in the host. FW-5 is editing
`present.rs`, `art_present/**`, `lanternjaw/**`, `scene.rs` concurrently:
read them, never write them; if you need a helper from there, copy the
logic into the adapter and cite the source line. Commit ONLY as
`git commit -m "..." -- <your paths>`, verify with `git show --stat HEAD`,
rebase onto the branch head as FW-5 lands, keep the workspace compiling at
every commit (the `cubarium-gpu` member line stays in; the crate must build
on the desktop without a GPU present at compile time).

## Deliverable

1. `cubarium_gpu::adapter`: `Scene::from_view(&RenderView, &Atlas, seconds,
   f)` producing the tick-rate `Fields` (revision = world tick) and the
   frame-rate layers exactly as the CPU presenter chooses them: ground
   cover density, plant slots with growth stage pairs and mix, tall
   columns with height, rain, bodies with heading/pose/tone, and hunters
   decomposed into one instance per rig part. Add the two extra frame
   slots so multi-pose stamps composite as the CPU does (decision made).
   `canopy_top` read from the config (FW-5 adds the key; until it lands,
   0.67 constant with a TODO naming the key).
2. `sink/gpu.rs`: a `FrameSink` that ignores `Output` bytes and instead
   takes the `RenderView` through a new optional hook
   `FrameSink::observe_view(&RenderView, seconds, f)` (default no-op; add it
   to the trait, it is object-safe), builds the `Scene`, renders, and
   presents via the socket (board) or the window (desktop), chosen by a
   `--gpu-target shim|window` flag defaulting to `shim` when the socket
   exists. The runner calls the hook where it calls the presenter today.
   `--sink gpu` refuses a cube world by name.
3. Fidelity check: a test that builds one `RenderView` (fixed seed, 3,000
   ticks, ring 320×180), renders it on the GPU headless and through the
   CPU presenter to PNGs, and reports mean |Δ| per channel and the share of
   pixels off by more than 8, with a stored threshold; run on the desktop.
   Where the two differ systematically (bend rounding, pose compositing,
   band edges), say which is right and why in the report.
4. Board: build as `particle` (or the `cubarium` user if W2 has created it)
   under `taskset -c 4-7`, run the command above for 5 minutes with the
   daemon's log and the host's log: fps, GPU ms, CPU core-seconds per
   second for the whole process (sim + adapter + render), RSS. Also 320×180
   S=1 and 960×540 S=3 for one minute each.
5. Wind: implement the sub-texel bend at S ≥ 2 behind a flag
   (`--gpu-bend-substep`, default off) so Wrysk can compare on the panel;
   the block rule stays exact for everything but the bend displacement.

## Constraints

- The CPU path and the cube are untouched; FW-5's files are read-only to
  you. No new crates beyond what Stage A pulled in. `ash` + `drm` only.
- Trailer `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No
  merge, no push. Same device rules as Stage A; the daemon stays running
  and you go through its socket.

## Return

Report at `design/7_Research/gs1b-live-world-2026-09-16.md` and as your
final message: the adapter's mapping table (CPU pass → layer/instance
fields), the fidelity numbers and the systematic differences with the
verdict on each, the board measurements at three rungs, what Wrysk should
look at on the panel, and what is left (`Mask::Radial`, anything skipped).
