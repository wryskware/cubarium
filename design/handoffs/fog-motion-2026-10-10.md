---
status: open
date: 2026-10-10
owner: renderer-worker, worktree `.claude/worktrees/fog-motion`, branch `fog-motion`
---

# FOG-MOTION: fog that never jumps, and moves

Rules: `design/handoffs/worker-rules-2026-10-07.md` (no windows, 5-minute calls, own
`target/` seeded by reflink, explicit-path commits, MP4 recipe, ≤ 40-line return). No
`unsafe` except the approved `ash` FFI calls inside `cubarium-gpu`, one call per block.
Worktrees and targets never go under `/tmp` (a RAM disk). Put every video you hand back
under the **main checkout's** `art/gen/runs/fog-motion-2026-10-10/` (absolute path), not
inside the worktree: ignored files die with it.

## Why

Wrysk (2026-10-10, watching his display world at night, desktop, lit tier): "when fog rolls
in or out, it just appears or dissapears in jumps. this makes the look not great and feel
glitchy. can we at least smooth it? would also be nice to have some animation, whether that
be randomly varying the intesity of each fog patch (perlin noise most likely) or subtle
motion queues to suggest flowing. also fog does seem to have some volumetric ish lighting
but can kindof read like simple alpha blended sprites at times."

## 1. Find and fix the jumps (a fix: animation never pops)

The sim's fog is one eased scalar (`crates/cubarium-voxel/src/weather.rs` `fn fog`), so the
jump is likely in how it reaches the screen. Find it from evidence, not guesses. Suspects:
the uniform updated only on some ticks or packs (`cubarium_gpu::weather::Look`, `fogK`);
the fog floor `fogK.y` (the sink's low ground) switching basin; the noise clock `wxTime()`
stepping with the sim; the light ladder quantising the fog's colour or coverage into rungs;
a threshold anywhere in `voxel.frag`'s fog (`fogShape`, `fogNoise`, `fogDepth`, the
weather overlay near line 1739). Check which path the desktop draws: `config/desktop/
terrarium.toml` (`lighting = "lit"`, `weather_speed` auto) and whether `VOLUMETRIC` is on.

Fix it so fog forms and clears continuously on the frame clock at the display's real
weather speed. That fix ships without asking.

## 2. Motion candidates (Wrysk picks)

Build these as switchable options (a `[light]`/fog setting, documented as a trial), each
on top of the fix:

- **a. Fixed only**: today's look, without the jumps.
- **b. Breathing banks**: each fog bank's density drifts up and down slowly and
  independently (a coherent noise over the ground plane and time).
- **c. Flowing**: (b) plus the banks drift slowly with the wind (`wind` in the weather
  state), so the fog reads as moving air.

Keep the speeds and amplitudes subtle and state them; Wrysk tunes from the video.

## 3. The "alpha sprite" read (diagnose; optional candidate)

Say in two or three lines why the fog can read as a flat alpha-blended sheet (for example,
one noise value per pixel where its ray enters the band makes a 2D sheet). If a cheap fix
exists (density varying along the ray, a soft edge where fog meets terrain), add it as
candidate **d**; otherwise only report it.

## Evidence

Headless, lit, real time, from `crates/cubarium/examples/light_capture.rs` or a sibling.
One MP4 of fog forming and clearing, today | fixed (a) side by side, plus one MP4 of
a | b | c (| d) side by side with fog held. Night and day, at the desktop's scale
(`px_per_voxel = "auto"` on 1920×1080, which is 6) and a 2× close-up. Speed the weather up
for the forming/clearing clip if you must, but say by how much, and show one stretch at the
display's real speed.

## Return

Commits; the cause of the jumps with its evidence; MP4 paths (absolute); each candidate's
setting name, speed and amplitude; frame cost on the desktop GPU; what still looks wrong.
Stop there: no merge, no deploy.
