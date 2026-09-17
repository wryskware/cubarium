---
status: leaning
date: 2026-09-17
owner: Fable
---

# Voxel rendering on the GPU: local window, then the Tachyon, then the cube

Wrysk (2026-09-17): the rendering engine runs **in parallel** with the ecology
rounds, on the GPU, so progress is visible long before deployment. Targets in
order: a **local window**, then the **Tachyon panel**, then the **LED cube**.

## What exists, and what this reuses

- `crates/cubarium-gpu`: raw `ash` Vulkan (no wgpu, no Mesa on the board), a
  world raster at `R8G8B8A8_SRGB`, a `present` pass (nearest ×k upscale plus
  quarter turn) into a window swapchain or a linear dma-buf for KMS. `Gpu::open`,
  `Renderer::record`, `PresentTransform::fit`, `read_raster`, the fullscreen
  vertex shader and the SPIR-V build (`shaders/compile.sh`) are all reusable.
- `crates/cubarium/src/sink/gpu/**`: `GpuSink` with `GpuTargetKind::{Shim,
  Window, Headless}`, `--gpu-capture DIR` PNGs, `--gpu-web-rate` viewer. The
  Window target reads the raster back into the desktop window each frame; Shim
  hands dma-bufs to `cube-screen-shim` on the board.
- `crates/cubarium/src/voxel/{project,present,stand}.rs`: the CPU voxel
  presenter. It is the **definition of the picture**: a vertical-shear elevated
  orthographic projection (`s` px per voxel, `rise` rows per voxel of depth),
  front and top faces, autotiled rim / bevel / chamfer / riser / contour rules,
  roof shadow, per-row haze, translucent free water with exactly one surface per
  pixel, pore-water darkening, plants as voxels (trunk column + crown disc),
  sprout glyph on seed banks. Its tests in `present.rs` name every rule.
- `cubarium voxel` run loop (`voxel/mod.rs`): 20 Hz world + flora, stdin
  controls, `--sink png|web`, `--speed`, `--fps`.

The CPU presenter stays: it draws for tests and is the reference. The GPU path is
a second renderer of the same picture, as the ring world already has.

## Design decision: the projection becomes a shader (Fable, 2026-09-17)

The voxel picture is drawn **per pixel by a slab walk in a fragment shader**, not
by uploading a list of faces. For a raster pixel `(sx, sy)` the shader walks
slabs `z = 0 .. depth` **near to far**; at each slab the projection
(`front_row(y, z)`, `top` rows above it) says which voxel `(x, y, z)` owns that
pixel and whether the pixel is on its front face or its top face. The first
solid face ends the walk; water faces met on the way blend front-to-back
(premultiplied). Neighbour lookups for rim, bevel, chamfer, riser, contour and
roof are texture fetches. The CPU's back-to-front ownership bookkeeping
(`nearer_owns`, clipping) exists only because the CPU paints without a depth
test; front-to-back per pixel gives the same ownership by construction. Port the
**rules**, not the bookkeeping.

Why this and not face instances: the per-frame CPU cost drops to one upload per
tick, the raster can be any scale (the panel can run `s` = 8 or 12 with the same
data), and the later per-display choices Wrysk wants — smooth water, sub-voxel
wind on crowns, soft shadow, AA — are shader options rather than presenter
rewrites. The visible effect now: **the same picture**, at 60 fps, in a window.

Data per tick (world 128×48×24 at the defaults, ~150k voxels):

- one 3D texture, one texel per voxel: material id, free-water fraction,
  pore-water fraction, plant part/style id (`R8G8B8A8_UNORM`, ~600 KB);
- one small table for plant styles (species palette, wilt, fill) and one for
  materials (strata colours), as uniform or texel buffers;
- the roof-gap table the CPU builds (`build_roof`) as a second texture if
  walking up the column in the shader is not cheaper; the worker measures and
  says which.

The plant stamping (`Stands::rebuild`) stays on the CPU and writes part ids into
the same voxel texture. The sprout glyph is a plant part like any other.

## Packages

**VR-1 — the voxel GPU renderer and the local window**, Opus high, worktree
from e32f276 (`voxel-render`). Files: `crates/cubarium-gpu/**` (new module
`voxel` with its shaders; `Renderer` gains a voxel pass or a sibling
`VoxelRenderer` that shares `Gpu`, the present pass and the targets),
`crates/cubarium/src/sink/gpu/**` and `crates/cubarium/src/voxel/mod.rs` (the
run gains `--sink gpu` with the existing `--gpu-target window|shim|headless`,
`--gpu-capture`, `--gpu-web-rate`), `crates/cubarium/src/cli.rs` for the
`VoxelSinkArg::Gpu` arm only. Deliverables:

1. `cargo run -p cubarium --release -- voxel --scene generated --sink gpu
   --gpu-target window` shows the live generated world in a desktop window at
   the run's `--fps`, with the stdin controls still working (`r`, `a`, `m`, `f`,
   `i`, `p`, `s`, `q`). The desktop has two NVIDIA GPUs under the official ICD;
   `Gpu::open` as it stands should find them.
2. **Fidelity example** `crates/cubarium-gpu/examples/voxel_fidelity.rs` (or a
   `cubarium` example if the world types are needed): render the authored scene
   (`voxel/scene.rs`) and the generated world at tick 0 and after 60 s of the
   default rain on both renderers (`--gpu-target headless`), write the CPU, GPU
   and diff PNGs into a directory, and print per-channel max and mean absolute
   difference. Exact match is not required. What **is** required: a checklist in
   the report, one line per presenter rule (rim, side bevel, chamfer corner,
   riser lean without rim, contour suppression on a receding plateau, roof
   shadow falling off with depth, per-row haze, water fill from the bottom,
   one water surface per pixel across a deep pool, partial roof clipping a
   water top, pore-water darkening, trunk shade, crown edge/underside, sprout),
   saying whether the GPU shows it and, where the pixels differ, why. A rule
   the GPU cannot reproduce is reported, not silently dropped.
3. **Measurements**: frame time on the desktop at the default projection and at
   `px_per_voxel` 8 (the panel-scale candidate), CPU per-tick upload cost, and
   the size of the per-tick upload. Short runs only.
4. **Screenshots** of the window at three moments (fresh world, after a rain
   pulse, after `a -20` lowers the aquifer) via `--gpu-capture`, named so the
   owner can find them, under `design/7_Research/assets/voxel-render/`.
5. Tests: short function tests only — the projection arithmetic in the shader's
   Rust twin (which voxel and face owns a pixel, for a handful of cases matched
   against `Projection::front_rect`/`top_rect`), the texel packing round trip,
   and the fidelity example runs to completion as a test **only if** a GPU is
   present (skip with a printed reason otherwise; never a hard CI dependency).

Do not build a winit path: the readback window (`GpuTargetKind::Window`) is the
local target this week. The `cubarium-gpu/window` workspace stays as it is.

**VR-2 — the panel**: later, once VR-1 lands: the same binary on the Tachyon via
`--gpu-target shim` (cubarium.service, `CUBARIUM_EXTRA_ARGS`), `px_per_voxel`
and raster height chosen so the strip fills 1920×1080 (owner's call; note the
candidate: 160 voxels wide at 4 px is 640 px, ×3 fills the panel exactly; the
world width is fresh-world config), measured fps on the Adreno 643.

**VR-3 — the cube**: later; the ring strip needs a cube projection choice first
(issue #5 "GPU cube"). Not designed here.

## Rules

Fast iteration; explicit-path commits; no cargo fmt; no long tests, no pinned
hashes, no golden PNGs (captures are disposable evidence); GLSL and its SPIR-V
checked in together via `shaders/compile.sh`; never enable
`drm_format_modifier` or touch the board's `DP-1` sysfs; do not touch
`crates/cubarium-voxel-flora/**`, `design/handoffs/README.md`, the cube, or the
running toy on port 7402; never HashMap iteration. Return the evidence: what was
measured, the checklist, the diff numbers, and every place the GPU picture is
knowingly different from the CPU one.
