---
status: open
date: 2026-10-10
owner: one worker per species, worktree `.claude/worktrees/feed-<species>`, branch `feed-<species>`
---

# FEED-ROLLOUT: a feeding clip for every eater, one species at a time

Rules: `design/handoffs/worker-rules-2026-10-07.md` (no windows, 5-minute calls, own
`target/` seeded by reflink from main's, explicit-path commits, MP4 recipe, ≤ 40-line
return). Blender runs `-b` only. Worktrees and targets never go under `/tmp`. Put every
video you hand back under the **main checkout's** `art/gen/runs/feed-rollout-2026-10-10/`
(absolute path), not inside the worktree: ignored files die with it. `unsafe` only for the
`ash` FFI calls in `cubarium-gpu` (you should need none).

## Why

Wrysk picked the bone rig with turned cubes (FEED-RIG, `design/handoffs/feeding-motion-rig-
2026-10-09.md`, merged 31db6437) and approved the Veilhound's clip: "veilhound animation
looks great!" (2026-10-10). Now each eater gets its own clip, drafted as a placeholder he
directs from video, one species at a time.

## The pattern (from the Veilhound, merged)

- The rig and clip live in the species' builder in `scripts/blender/animal_models.py`
  (`chorister()`, `ch_feed_armature`, `ch_feed_clip`, `ch_feed_angles`, `rig_armature`,
  `sample_clip`): every built part on one bone, the action sampled into the species' `.rig`
  side file at bake. Every baked size bin carries its bones.
- The engine is done: bone index in `FigureVoxel.bits` 14+, `bonePose` in `figure.vert` and
  `figure_sun.vert`, the clip evaluated on the frame clock, its weight easing with
  `State::Cropping`, each animal on its own loop phase. Touch it only if a species needs
  something it lacks, and say what.
- Evidence comes from `crates/cubarium/examples/voxel_feed_capture.rs`: two grown animals
  feeding, side-on and three-quarter, at the desktop's 6 px and 12 px, lit, real time,
  **today (no clip) | the clip** side by side. Report cells per bone.

## Species sections

Each section is one package. The clip is a believable placeholder; Wrysk directs the real
choreography from the video. Don't invent body parts, colours or new poses beyond it.

### Loftstrider (first)

The tall browser (2.2 × 2.2 × 3.4 m adult, feeding 0.8–3.5 m up, "a sloppy feeder"). Its
builder is `loftstrider()` / `loftstrider_rig(p)` / `build_loftstrider`, with the chibi
calf blended in by `ls_blend`; the rig must hold for the calf bins too.

- **Bones**: root (body, legs), a chain of 2–3 along the neck, a head bone (head and mouth
  parts). Report the count per bone, calf and adult.
- **Clip (placeholder)**: browsing a crown above it.
  1. Reach: the neck lifts and the head goes up and forward into the browse band, 0.6 s,
     eased.
  2. Strip: the head pulls back and down about 10 cm with a small sideways tug, alternating
     sides, 0.5 s; then reaches in again, 0.4 s.
  3. Every second strip, chew: the head draws back level with a small bob, 0.6 s.
  4. Loop while it crops; the reach and return come from the clip weight.
- Find whether the sim knows, per bout, how high the cell it crops is. If it is cheap to
  reach, say how the reach could aim at it (don't build it); the placeholder reaches a fixed
  height in the band, stated.
- Evidence as above, at a vaulttree if `voxel_feed_capture` can place one cheaply (say if
  not), adult and calf.

Next sections (written when each starts): frondgrazer (its neck under the shield is an
open look question), seedporter, lanternjaw, bellwing, capgnawer, littershredder,
ripple-snail. Then sleep, idle and hunker move onto the rig.

## Return

Commits; cells per bone; the MP4 paths (absolute); what looks wrong (joint gaps, tile
clipping, shadows, the calf); anything the engine lacked. Stop there: no merge.
