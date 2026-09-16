---
design_status: exploration
last_reviewed: 2026-09-16
---

# GS-1 (Opus, high): `cubarium-gpu`, the Vulkan renderer for the panel

Read, in order: `design/7_Research/gpu-scanout-spike-2026-09-16.md` (the
stack and the traps), `design/7_Research/presenter-budget-2026-09-16.md`
(§2's pass table is your shader list, §4(4) the upload budget),
`design/flat-world-plan-2026-09-16.md` §3 and §5a (the ring world and its
view), `design/appearance.md` and `design/game-art-workflow.md` (the look),
and `crates/cubarium/src/art_present/mod.rs` via `graft skeleton` (what the
CPU presenter draws, pass by pass). Fresh context. No nested agents.

## Objective

A new crate `crates/cubarium-gpu` that renders a ring-world scene on the
GPU with the same pixel-art look as the CPU presenter, into a ring of
linear `B8G8R8A8` images exportable as dma-bufs on the board, and into a
desktop window for development. Stage A (this package) renders a **synthetic
scene** through a `Scene` type you define; Stage B (later, after FW-2) adds
the adapter from the world's `RenderView`.

## Where

- Cubarium worktree `/home/wrysk/wryskware/cubarium/.claude/worktrees/tachyon-screen`
  (branch `tachyon-screen`). Never edit the main checkout. Other workers
  own `crates/cubarium-surface/**` (FW-1), `vendor/**`, `crates/cubarium/**`;
  you own only `crates/cubarium-gpu/**`, its workspace member line, and
  shader files under it. Rebase onto the branch head as others land.
- Device: same grant as GS-2's brief (build natively under `/root/cubarium`,
  `taskset -c 4-7`, stop/start `cube-screen-shim` around scanout tests, never
  write connector status or debugfs). GS-2 is concurrently adding the
  daemon's dma-buf socket; until it lands, test scanout the way the spike
  did (stop the daemon, flip directly), then switch to the socket.

## Deliverable

1. `Scene`: fields as `W×H`-cell textures (producer, water depth, detritus,
   growth stage, tall height, rain) uploaded per tick; instances per frame
   (organisms: anchor, heading, sprite id, frame pair, mix, tone; ground
   cover and plants: anchor, stage pair, mix, bend); `seconds` and the
   tick phase for animation; the raster size and scale `S`.
2. Pipelines, matching FW-P's table: one full-screen background pass over
   the field textures (floor, producer ramp, horizon fade, soil, detritus
   flecks, water with shimmer); instanced atlas quads for ground cover,
   plants, tall columns, bodies, hunters, rain; nearest-neighbour sampling;
   integer upscale to the panel and the 90° rotation in the final pass, on
   the board; sRGB via an `_SRGB` attachment. Pixel-art look: every sprite
   texel lands on an `S×S` block, no filtering, no sub-pixel positions
   except the `bend` displacement, which is quantised to whole source
   pixels as the CPU presenter does (read `art_present/tall.rs`).
3. Assets: load the existing sprite pack (`assets/atelier`, `pack.json`)
   into an atlas at S = 1 with an integer `scale` factor; no re-bake.
4. Targets: `Target::Window` (desktop, `winit` + `ash` swapchain) and
   `Target::Scanout` (board: linear images + dma-buf export as in the
   spike; flip directly until GS-2's socket lands, then through it).
5. A `synthetic` example scene (ring 320×180, S = 1 and 640×360, S = 2):
   a field gradient with water pools, a few hundred plant instances across
   growth stages, a dozen bodies moving on the ring, rain over a band,
   animated at 60 fps, wrapping visibly across the seam.
6. Measurements on the board at 1080×1920: GPU time per frame, CPU
   core-seconds per second, fps sustained over 60 s, for both scene sizes.
   Golden-image test: the synthetic scene at a fixed time rendered to a
   readback buffer compared with a stored PNG with a small tolerance, run on
   the desktop.

## Constraints

- `ash` 0.38, `drm` 0.15, `winit` for the desktop path only; shaders in
  GLSL compiled at build time (`shaderc` is acceptable if it builds on the
  board without apt; otherwise pre-compiled SPIR-V checked in with the
  source beside it). No wgpu, no Mesa, no DRM-modifier extension.
- The CPU presenter and the cube path are untouched.
- Commit small on `tachyon-screen`, trailer
  `Co-Authored-By: Claude Fable 5.1 <noreply@anthropic.com>`. No merge, no
  push.

## Return

Report at `design/7_Research/gs1-vulkan-renderer-2026-09-16.md` and as your
final message: the `Scene` type (the interface Stage B adapts to), the
pipeline list, the board measurements, a desktop screenshot path, what was
verified where, and anything left.
