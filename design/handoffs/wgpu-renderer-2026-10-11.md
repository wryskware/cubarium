---
design_status: exploration
last_reviewed: 2026-10-11
decision_refs:
  - D-0005
---

# Renderer backend migration

Wrysk requested the complete ash-to-wgpu migration, in a new worktree, with
renderer parity verified by screenshots. This is an execution handoff; it does
not add ecological or visual design decisions. Backlog item 11 supplies history.

Worktree: `.claude/worktrees/wgpu-renderer`, branch `codex/wgpu-renderer`.
Source baseline: `48dd6600`. The additive capture harness and pan control are
committed at `f6d55be5`; its rendering algorithms are the ash baseline.
The final rebase includes `main` at `69316ce7` and its merged `pipeline-warm`
work. The matching ash capture tree is `b1867d30` (tag
`wgpu-ash-main-2026-10-11`): that main revision plus the same additive pan
control. Main changed the animal save format, so its final comparison freezes
a fresh matching world rather than migrating the old schema-78 fixture.

The picture follows D-0005 and
`design/art-direction/Cubarium_Art_Direction_v0.1.md`. Preserve the cutaway
projection, pixel raster, nearest presentation, materials, illumination,
occlusion, reflections, weather, plants, animal interpolation and bloom.
No new repository unsafe code is allowed. The completed GPU crate must carry
`#![forbid(unsafe_code)]`; unsafe within external dependencies is excluded by
the working policy. Hardware ray queries are absent from this baseline.

## Work sequence

1. Freeze ash screenshots before changing rendering algorithms. Use the same
   saved world and explicit presentation clocks for both backends.
2. Prove the safe wgpu device, shader input, presentation and padded readback
   route before porting world passes. Use wgpu 30.0.1. Naga rejected twelve
   baseline SPIR-V modules; convert their GLSL algorithms to WGSL through its
   GLSL frontend, preserving specialization overrides. Wrysk prefers maintained
   WGSL when this does not require a substantial manual rewrite. Validate the
   generated WGSL before adopting it as the shipped source.
3. Port flat background/water/sprites and voxel sky, figure shadows, voxel
   walk, plant visibility/shading, figures/composite, weather and bloom.
   Preserve CPU-facing scene/packing representations and specialization values.
4. Replace Vulkan handles at the FrameSource boundary with safe textures,
   command encoders and target views. Preserve recording discard, capacity,
   retirement, version tags and ordered, nonblocking Godot readback.
5. Port standalone and application window presentation and legacy panel
   targets. Ordinary safe wgpu does not expose the old Vulkan dma-buf export:
   the panel fallback uses completed readbacks copied into safely allocated DRM
   buffers, then the existing shim handoff. This retains output pixels and
   release ownership, but adds CPU copy cost. The panel no longer constrains
   backend choice (backlog item 11 / D-0005).
6. Remove direct ash dependencies and Vulkan-only resource code. Check native
   platform conditionals, shader portability and both default/no-scanout builds.
7. Preserve main's Still/All preview policies, parallel walk builds, NVIDIA
   first-compile guard, queued lighting/fade and cancellable async warming.
   Hosted previews, live worlds and their warm worker share an `Arc<Gpu>`.
   Shader modules, layouts and pipelines are interned on that device; concurrent
   requests for one descriptor compile once. Safe wgpu cannot construct its
   native disk cache without unsafe, so this cache lives until the device drops.
   Desktop windows retain display-compatible adapter selection.

## Verification

All builds, tests and capture commands go through labd. Keep Rust builds at
eight jobs or fewer, use the common build cache and small headless fixtures.
An agent window always launches through `scripts/hidden.sh`.

`scripts/gpu-parity.py` captures ring scale 1/2; plain/textured terrain and
water at day/dusk/night; rain/fog; two water clocks; a frozen pan; and specimens
with plants, models and glow. Additional existing fixtures cover fractional
animal motion and WindowFit upscale, letterbox, downscale and portrait output.
The comparator reports changed pixels, maximum and mean channel error, and
writes differences. Review the pictures and numerical errors. Wrysk accepted
the tiny 1–2/255 rounding differences on 2026-10-11; do not chase every last
pixel, silently widen a tolerance or replace a baseline to make a check pass.
The strict comparator deliberately still returns 1 when any pixel differs.

Exercise asynchronous readback ordering, backpressure, retirement, slot reuse
and dropped recording behavior with a tiny asymmetric texture. Run tests for
each changed crate and one integrator workspace pass. Inspect screenshots in
addition to numerical differences. Verify Metal/HLSL shader translation where
available; a Linux capture does not establish a macOS or Windows runtime pass.

Keep the final requested screenshot pairs and concise comparison result under
`runs/wgpu-parity-main/`. Source history is commits, not frozen binaries or build
copies. Update backlog/status after the full migration and verification pass.

Final verification is in progress. The pre-rebase wgpu revision `b14697ff`
passed all 139 GPU tests, Linux workspace checks, Windows main/Godot cross-checks,
36 shader cases with MSL/HLSL output, and a three-frame real surface test on a
private Xvfb display. Its 45 screenshot pairs contained 4,836 changed pixels
out of 13,417,632 (0.036%), with maximum channel error 2/255. Final checks also
exercise concurrent cache reuse, device isolation and Still-to-All warming.

After the final rebase, all 140 GPU tests pass (65 s, no skips); the Linux
workspace and no-scanout checks pass. The actual device cache probe confirms
one creation for eight concurrent requests, separate caches on separate
devices, six new variants for Still-to-All, no new variants for repeated All,
and distinct presentation pipelines for distinct output formats. The GPU
source tree is identical at `61017541`, `dc439d11` and `5116f722`; the latter
commits fix a test import and expose read-only diagnostic counters.

Physical panel output and native Windows/macOS runtime remain untested here.
