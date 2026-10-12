---
design_status: exploration
last_reviewed: 2026-10-11
decision_refs:
  - D-0005
---

# Renderer backend migration

Implemented on `codex/wgpu-renderer` in `.claude/worktrees/wgpu-renderer`.
Wrysk requested complete renderer parity, maintained WGSL where practical, and
screenshots. This records implementation and verification, not new canon.

The GPU crate uses wgpu 30.0.1, maintained WGSL and `#![forbid(unsafe_code)]`.
Direct ash resources are removed. Flat/voxel, water, figures, plants, lighting,
weather, bloom, presentation and tagged asynchronous readback retain their
existing CPU interfaces and visual behavior. Encoder-owned uploads preserve
discard, bounded capacity and retirement. Windows choose a display-compatible
device and draw/present inside Winit callbacks. Agent windows use `hidden.sh`.

Main's merged `pipeline-warm` is ported: Still/All policies, parallel walk
builds, NVIDIA's first-compile guard, queued preview lighting/fade, cancellation
and teardown remain. Godot previews, live worlds and warm-up share `Arc<Gpu>`;
per-device modules, layouts and pipelines are interned with concurrent
same-descriptor creation deduplicated. Normal desktop windows retain their
surface-compatible device selection.

The original renderer was frozen at `f6d55be5` (base `48dd6600` plus additive
capture controls). After rebasing, main's animal save format had changed. The
final ash baseline is `b1867d30`, tag `wgpu-ash-main-2026-10-11`: main at
`69316ce7` plus the same additive pan control. A fresh world was frozen there;
no old save migration or baseline edits were used to obtain a match.

`runs/wgpu-parity-main/index.html` is the standalone comparison gallery.
`comparison.json` reports 45 pairs, 13,417,632 pixels, 4,836 changed pixels
(0.03604%) and maximum channel difference 2/255. The pictures were also
inspected. Wrysk explicitly accepted the tiny rounding differences; the strict
comparator still exits 1 whenever any pixel differs. Coverage includes ring
scales, day/dusk/night, plain/textured terrain, rain/fog, two water clocks, pan,
plant/model/glow specimens, classic/crisp/smooth animal motion, upscale,
letterbox, downscale and portrait presentation.

`source.json` records commits, the GPU tree and frozen input hashes. Capture
revisions `dc439d11` / `5116f722`, cache revision `61017541`, and final Rust
verification revision `3650682e` have the same GPU source tree. Main/Godot
verification includes the final cross-platform explicit-save import fix.
The final rebase includes main at `9455ef0c`, including its later Godot chrome
and fauna fixes. These introduce no GPU source changes.

All Rust builds, tests and captures ran through labd, with at most eight build
jobs and serial GPU tests. Completed checks:

- 140 GPU tests passed (65 s); 978 main/Godot tests passed (366 s); no skips.
- Linux workspace/all-target and no-scanout checks; Windows main/Godot
  all-target cross-check.
- 36 maintained WGSL cases, MSL/HLSL output and real Linux pipeline creation;
  padded readback, ordered tags, slot reuse and discarded upload.
- Cache probe: eight concurrent requests create one pipeline; device isolation;
  Still-to-All adds six variants; repeated All adds none; format separation.
- Actual hosted fast preview and shared-device warm reuse; main headless and
  private-window startup/shutdown; three-frame real private-display surface.
- Device create/drop cycles and eight independent panel mapping/rotation cases.

The final source checks found and fixed a test trait import, a diagnostic's
private field access and Unix-gated imports needed by Windows app saves. A
lab cache seeding collision was avoided by reusing the existing snapshot target
and Cargo's build lock. The full integration test ran in the normal lane after
its cold build exceeded the quick lane's 15-minute limit.

Material limits: native pipeline disk-cache construction requires unsafe, so
our cache is process-resident; the driver may maintain its own disk cache. The
legacy Linux panel copies completed readback into safe DRM buffers before the
existing shim handoff, adding CPU copy cost. Full voxel rendering requires 21
sampled textures per shader stage; a weaker window adapter uses the existing
headless-GPU/CPU-window fallback. Physical panel output and native Windows/macOS
runtime are untested here. MSL/HLSL emission and a Windows cross-check are not
native runtime verification.

Smoke commands create their own temporary world copies and remove them on exit.
Only the final requested pairs and frozen inputs are retained. Superseded
captures and smoke worlds are task-owned temporary data; source history lives
in Git commits and lightweight tags. Backlog item 11 records the original
renderer deferral. No ecological or visual decisions were promoted.
