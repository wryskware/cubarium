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

## Verification

All builds, tests and capture commands go through labd. Keep Rust builds at
eight jobs or fewer, use the common build cache and small headless fixtures.
An agent window always launches through `scripts/hidden.sh`.

`scripts/gpu-parity.py` captures ring scale 1/2; plain/textured terrain and
water at day/dusk/night; rain/fog; two water clocks; a frozen pan; and specimens
with plants, models and glow. Additional existing fixtures cover fractional
animal motion and WindowFit upscale, letterbox, downscale and portrait output.
The comparator reports changed pixels, maximum and mean channel error, and
writes differences. Investigate every mismatch; do not silently widen a
tolerance or replace baseline images to make a check pass.

Exercise asynchronous readback ordering, backpressure, retirement, slot reuse
and dropped recording behavior with a tiny asymmetric texture. Run tests for
each changed crate and one integrator workspace pass. Inspect screenshots in
addition to numerical differences. Verify Metal/HLSL shader translation where
available; a Linux capture does not establish a macOS or Windows runtime pass.

Keep the requested screenshot pairs and concise comparison result under
`runs/wgpu-parity/`. Source history is commits, not frozen binaries or build
copies. Update backlog/status after the full migration and verification pass.
