---
design_status: exploration
last_reviewed: 2026-10-12
decision_refs:
  - D-0005
---

# WGSL-SOURCE: hand-written WGSL becomes the one shader source

Wrysk, 2026-10-12: "id kindof rather wgsl be the source of truth. its easier for me to read."
Today the WGSL the program runs (`crates/cubarium-gpu/shaders/wgpu/*.wgsl`) is naga's machine
translation of the GLSL. It's 50k lines against the GLSL's 4.9k, with thousands of `_e198`-style
temporaries, and `world.glsl` and the other shared files are pasted into about five shaders. The
GLSL, the `.spv` files and `compile.sh` still sit beside it, and editing them changes nothing
(hosts review H1: `design/handoffs/wgpu-review-2026-10-11/hosts.md`).

## Objective
Port the GLSL by hand into readable WGSL that a person maintains, make it the only shader
source, and delete everything else: the GLSL files, the `.spv` files, `compile.sh`, the generated
`shaders/wgpu/*.wgsl`, the unused present copies, and the GLSL → WGSL generator path
(`Shader::wgsl()`, `wgpu_probe --write-wgsl`) once nothing needs it.

## What readable means
- Written as a person writes WGSL: the GLSL's function and variable names kept so history maps
  across, its comments carried over, no SSA copy chains or `_eNNN` temporaries.
- Shared code written once as shared modules (`world`, `scene`, `plant`, `figure`, `volumetric`,
  `drape_cord`, `plant_main`), not pasted into each shader. Expect roughly the GLSL's size.
- The variants `compile.sh` built from `voxel.frag` with `-DFOG_OCCLUDE`, `-DPLANT_VIS` and
  `-DPLANT_SHADE` (the weather walk and the two plant fragment stages) need a WGSL form. Pick
  one (module composition, separate entry points, or override constants) and say why. Pipeline-
  overridable constants make wgpu run naga's override pass for each pipeline, so weigh that cost.
- WGSL has no includes. Pick the simplest safe mechanism that keeps files readable and works in an
  editor: a small import resolver in `wgpu_backend/shader.rs` (the GLSL path already had a
  preprocessor), at load time or in a `build.rs`, or WESL / naga_oil if clearly better. Say why.

## Repo and boundaries
- Repo `/home/wrysk/wryskware/cubarium`. Your worktree and branch: `wgsl-source`
  (`.claude/worktrees/wgsl-source`, off main, own reflink-seeded `target/`).
- You own `crates/cubarium-gpu/shaders/` and `crates/cubarium-gpu/src/wgpu_backend/shader.rs`
  (also L3 in the hosts review: interning copies and hashes the whole source on every request),
  plus shader tests and the shader-related examples.
- **Shader interface frozen**: bind group layouts, binding numbers, struct layouts, entry point
  names, vertex inputs and outputs stay exactly as at `ce035f3a`. A parallel package
  (PERF-RESTORE, branch `perf-restore`) is changing the Rust renderer and relies on that.
  Don't touch Rust outside the files you own; if something truly needs it, stop and say so.
- No look changes. Wrysk decides every look.
- Update every doc that tells people to edit GLSL or run `compile.sh` (at least
  `design/handoffs/{voxel-render-briefs-2026-09-17,presentation-plan-2026-09-24,fog-occlude-2026-10-11,cell-shrink-2026-10-10,glow-ease-2026-10-10,drape-cord-2026-10-10}.md`,
  and README / WORKING_POLICY / AGENTS.md if they mention shaders) so they point at the WGSL.
- No `unsafe` anywhere, including `create_shader_module_trusted`.
- Never write the word "chorister" anywhere, file names included.

## Verification (the acceptance)
- The existing shader validation tests (the 36 WGSL cases, MSL/HLSL output, real pipeline
  creation) and the GPU tests pass.
- **Pixel parity with `ce035f3a`**: run the parity capture matrix (`scripts/gpu-parity.py`; the
  `scripts/gpu-parity-*.json` specs hard-code `.claude/worktrees/wgpu-renderer`, so make
  worktree-independent copies; frozen inputs in `art/gen/runs/wgpu-parity-2026-10-11/fixture/`).
  Aim for identical. Report changed-pixel counts and the largest channel difference for each pair,
  and explain any difference (float reassociation can move a channel by 1/255; anything more is a
  bug). Put the comparison gallery under `art/gen/runs/wgsl-source-2026-10-12/`.
- GPU ms per pass not worse than `ce035f3a`: a short headless timing comparison (the finish line
  of `cubarium voxel --sink gpu --gpu-target headless` prints the GPU splits), before and after,
  on the desktop's idle RTX 5090 (`CUBARIUM_GPU=5090`). Hand-written WGSL may well be faster.
- Shader file sizes before and after (`wc -l`).

## How to run things
- **No command of yours may take longer than 5 minutes.** Wrap anything that could in
  `timeout 270`. Never sleep, poll or wait on a job. A detached chain (full GPU tests, the parity
  matrix) runs straight through under one long timeout, ending in a `DONE`/`FAIL` marker, and you
  hand back while it runs.
- Quick compiles and targeted tests run directly in your worktree (`~/.local/bin/cargo` applies
  `-j 8` and idle priority; never a bigger `-j`). Full suites and the parity matrix go through labd
  (`scripts/lab/lab submit SPEC`, `design/lab-daemon.md`; GPU tests serially) or a detached script.
- Windows: never open a window on the desktop. Headless captures only (`--gpu-target headless`,
  `--gpu-capture DIR`); anything that needs a window runs under `scripts/hidden.sh`.
- No stress tools or synthetic load on the desktop. Don't use voidrunner (a benchmark runs there).
- Commit on `wgsl-source` with explicit paths (never `commit -a`). Never merge, rebase main,
  deploy, or touch other worktrees.

## Hand-back (at most 40 lines)
- Branch head; the module layout and the include and variant mechanisms you chose, with why.
- Line counts before and after; test pass/fail counts; the parity output per pair; GPU ms
  before and after.
- What is still running, where, and its markers.
- Anything you couldn't port faithfully, and decisions the parent should check.
