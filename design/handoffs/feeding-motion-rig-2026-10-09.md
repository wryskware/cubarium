---
status: open
date: 2026-10-09
owner: renderer-worker, worktree `.claude/worktrees/feed-rig`, branch `feed-rig`
---

# FEED-RIG: how animals get authored motion (trial on the frondgrazer's feeding)

Rules: `design/handoffs/worker-rules-2026-10-07.md` (no windows, 5-minute calls, own
`target/` seeded by reflink, explicit-path commits, MP4 recipe, ≤ 40-line return). No
`unsafe` except the approved `ash` FFI calls inside `cubarium-gpu`, one call per block
(WORKING_POLICY "No unsafe code").

## Why

Wrysk (2026-10-09, art thread): "i want to add feeding motions to animals that are eating
... we can take the blender block frame models, rig and animate them, then turn those into
voxel frames. if there's an efficient way to rig/animate the voxel models, we could try
that ... this new method could back-apply to animations we've already done like the
idle/sleep/hunkering."

The models are small on the grid: an adult frondgrazer is 81 cells, about 6 voxels long,
its neck and scoop a handful of cells. A per-frame voxel model (a flipbook) can only move
its head a whole voxel at a time, about 13 px on the desktop, while everything else in
the figure pass already moves sub-voxel (the glide, the free turn, the sleep settle and
breath, the idle head dip, the hunker). So this package tries both, in the real renderer,
and Wrysk picks from video.

## What to build

One Blender rig and one feeding clip for the frondgrazer, drawn three ways:

- **A. Flipbook.** The clip re-voxelized at every sampled frame (30 fps) with the existing
  bake rule (`voxelize_animal`), drawn by swapping the model per frame. Harness only: do
  not ship a file format for it. Report the distinct frame count and total cells per bin.
- **B. Bone rig, turned cubes.** The rest model baked once, each cell carrying the index of
  the bone it belongs to. Per drawn frame each bone's transform (from the same Blender
  action) is applied in `figure.vert` to the cube's corners, so cubes turn with their part.
- **C. Bone rig, upright cubes.** The same bones, the transform applied to each cell's
  centre only; the cube stays square to the body's frame.

B and C are one engine path with one switch (a trial setting, e.g. `[figures] rig =
"turn" | "upright"`, documented as such). Whichever Wrysk picks becomes the shipped path
later; A is only the comparison.

### The rig (Blender)

- Build a real armature in the script (`scripts/blender/animal_models.py`, the frondgrazer
  builder): a root bone for the shield, a chain of 2–3 bones along the neck, a scoop/head
  bone, the eye stalks under the head. Every built part belongs to one bone; split the
  neck tube by bone or give each neck cell its nearest neck bone, your call.
- The voxelizer records each cell's bone the way it records its material.
- The action is the one source of the motion for all three arms.

### The clip (a placeholder, for judging the method)

Wrysk will direct the real choreography later; this one only has to be a believable graze.

1. Reach: from the rest pose the neck lowers and reaches forward until the scoop's lip is at
   the ground line about one voxel ahead of the front feet, 0.4 s, eased.
2. Two bites: the scoop dips about 3 cm and tips forward about 15°, then back, 0.35 s each.
3. Chew: the head lifts about 4 cm and the scoop tilts back a little, 0.5 s.
4. Loop bites and chew while the body crops. On stopping, return to rest over 0.4 s.

The eye stalks ride on the head, counter-turned so the eyes stay roughly level.

### The engine (B and C)

- The bone index rides in `FigureVoxel.bits` (bits 14 and up are free). Per figure, the
  frame's bone transforms go to the GPU alongside the tile (a new buffer or tile fields).
  `figure_sun.vert` applies the same transform, so shadows follow.
- The host evaluates the clip per figure on the **frame clock** (not the tick), like
  `PoseSpec::at`. A per-animal clip weight eases 0 → 1 while `State::Cropping` holds and
  back after, as the meal flash's blend does (`voxel/animal.rs`, `flash_blend`). Each
  animal's loop phase is its own, so two grazers are out of step. Nothing pops.
- The tile box grows to hold the clip's reach. Faces between cells of different bones are
  drawn, not culled as interior, so a bending joint doesn't open a hole.
- The format: the smallest change that keeps the bake the one source (a bone byte per cell
  in a new CVM version, with a side file for the sampled clip, is fine). The library is
  rebaked, never migrated.
- Sleep, idle and hunker stay on their class path untouched. No other species.

## Evidence

Extend `crates/cubarium/examples/voxel_meal_capture.rs` (or a sibling example): the same
bout script, two grown frondgrazers per arm, **A | B | C side by side in one MP4**, lit,
real time:

- side-on, and a second clip at a three-quarter heading, because the free turn
  re-rasterises;
- at the desktop's scale (what `px_per_voxel = "auto"` gives `config/desktop/terrarium.toml`
  on 1920×1080; state the number) and one 2× close-up.

## Checkpoint: stop after the MP4

Return: commits; the MP4 paths (absolute); the frame cost of the bone path with 200
animals feeding; the flipbook's frame and cell counts; what looks wrong in each arm (joint
gaps, tile clipping, shadows); defaults you chose.

Not in this package: other species, back-applying sleep/idle/hunker, merging, deploying.

## Round 2: the Veilhound (2026-10-09)

Round 1 (branch `feed-rig`, 230f8f61) built the rig path, the `.rig` side file, the trial
switch and `voxel_feed_capture`. Its frondgrazer was too weak a test: only 5 of the adult's
81 cells sit on moving bones (the shield claims the neck base), and at the desktop's 6 px
a voxel the reached head hides behind the front hoof. The arms barely differ on screen.

Round 2 runs the same three arms on the **Veilhound** (code name `chorister`, adult bin
1.4 m, 142 cells): its skull, crest lobes, eyes and muzzle are a real head.

- **Rig**: `chorister()` in `animal_models.py`. Bones: root (body, legs, tail), a neck
  bone carrying the front of the spine loft (the cells ahead of the shoulders, by position:
  the loft is one mesh), a head bone carrying the skull, crest lobes, eyes and muzzle. Every
  head and neck cell has to land on its bone; report the count per bone.
- **Clip (placeholder, Wrysk directs the real one)**: feeding at a carcass on the ground.
  1. Reach: the neck drops and the head lowers until the muzzle is at the ground line about
     one voxel ahead of the front paws, 0.4 s.
  2. Tear: the head pulls back and up about 4 cm with a small sideways twist, alternating
     sides, 0.4 s; then dips again, 0.3 s.
  3. Every second tear, a swallow: the head lifts about 8 cm, 0.4 s.
  4. Loop while the body crops. The reach and the return come from the clip weight, as in
     round 1.
- **Evidence**: as round 1, A | B | C side by side, side-on and three-quarter, at px 6 and
  px 12. No carcass prop needed.
- Same branch and worktree, on top of 230f8f61. The frondgrazer's clip stays as it is; its
  neck is an open look question for Wrysk.
