---
design_status: exploration
last_reviewed: 2026-09-28
decision_refs: []
---

# Handoff: rigged voxel animals (skeleton → clips → baked voxel poses)

Wrysk, 2026-09-28: "are we using like proper 3d models and rigging..? cuz that seems like
the right way to do it". The answer today is no. This note is for the later session that
takes it on. How animals look is Wrysk's thread (art direction); nothing here decides the
look.

## Where things stand (main 3ccc6daf)

- **Source shapes.** Every animal is built procedurally in Blender from primitives in
  `scripts/blender/organism_lineup.py`: `frondgrazer`, `littershredder`, `bellwing`,
  `lanternjaw`, `loftstrider` and `chorister`. They are cylinders, ellipsoids and boxes,
  with no armature.
- **The bake.** `scripts/blender/bake_voxel_models.py` voxelises each founder in 5 size
  bins, from newborn to adult. Each is **one pose, facing +x**, stored in
  `assets/voxel-models/<voxel_mm>/<name>.cvm`. The format and tags are documented at the
  top of `crates/cubarium/src/voxel/model.rs`; cells are `dx dy dz material tag`, 5 bytes
  each. Numbers come only from `assets/voxel-models/source.json`, which
  `examples/voxel_model_source.rs` writes.
- **Two draw paths.** The baked model is one (`model::each_animal_cell`). The other is the
  hand-coded glyph (`animal::cells_of`, with parts such as `AnimalPart::Head`), which is the
  dev path and the fallback. Every organism must keep a plain-voxel path
  (`voxel-textures-experimental`).
- **Heading.** The model is rotated by `pose.heading_rad` when it is stamped each tick, so a
  small model re-voxelises on every turn.
- **Motion today.**
  - Locomotion re-stamps the whole body at its new cell each tick (20 Hz). Legs don't move.
  - **Sleep** (`voxel/sleep.rs`, 339a3eb1) and **idle** (`voxel/idle.rs`, b716c213 /
    8957d03f) are shader tricks. Each posed cell carries a class (torso, back, leg, head)
    and a pose-entry index in the texel's spare byte. `voxel.frag` then draws those cells
    shifted by whole **pixel** rows, from a per-frame pose table.
    - This gives smooth sub-voxel breathing, settling and head dips.
    - It **cannot** bend a joint. The idle "glance" broke on side-facing bodies, where the
      head detached from the neck, and was dropped for this reason.
- **Rules it must keep.**
  - Animation never pops: continuous motion on the frame clock.
  - The flat (panel) tier steps at its tick redraw with sub-voxel steps and no extra
    redraws.
  - Pixel-art illustration stays blocky, while motion and effects may be smooth.
  - No windows; Blender runs as `blender -b`.

## The proposal (to be refined in that session)

1. **Rig.** Add an armature to each animal in Blender: spine, neck, head, limbs, tail or
   wings as the species needs. Either rig the existing lineup builders or replace them with
   authored meshes (the `.blend` becomes the source of truth for shape). Size bins stay,
   either through armature scale or one rig per bin.
2. **Clips.** Author short actions per species. A starting set:
   - `idle` (breath and head turn), `walk`, `eat`/`crop`, `sleep_enter` / `sleep`, `climb`;
   - later: `drink`, `startle`, and death/limp;
   - the species with special motion: bellwing `fly`, seedporter `glide`/`climb`, the
     lanternjaw's strike and lure (`design/predation-and-flight-2026-09-28.md`).
3. **Bake poses, not frames.**
   - Voxelise a few **key poses** per clip, about 2–8, per size bin and per heading octant,
     into an extended `.cvm` (CVM2: `clip, pose, heading` added to each step).
   - At 3–10 voxels of body, a raw voxelisation of an in-between pose flickers, so key poses
     are chosen, and possibly hand-tidied, like pixel-art frames.
   - Bake per heading octant instead of rotating at stamp time. That removes today's
     turn-aliasing and lets the art tidy each facing.
4. **Play.** The presenter picks the clip from the fauna `State` and pace (Walking and speed,
   Cropping, Resting, Asleep, Climbing), and the key pose from the clip clock.
   - **Between key poses:** reuse the sleep/idle machinery. Cells carry a bone class, and the
     shader slides them by sub-voxel pixel offsets toward the next pose, so transitions stay
     continuous and never pop.
   - The per-cell class widens from "torso/back/leg/head" to a bone index.
5. **Keep.** The glyph path stays as the dev fallback, with the current shader poses. The CPU
   presenter stays basic and may draw the first key pose only.

## Decisions for Wrysk (open)

- **Source of truth for shape:** rig the procedural lineup, or move to authored `.blend`
  meshes, which are editable by hand.
- **How much hand work:** fully automatic voxelisation of key poses, or an art pass that
  tidies them (a pixel-art-style frame edit, possibly in a small voxel editor).
- **Clip list and priority per species.** The founders first, or the flight and predation
  species first?
- **Heading octants vs free rotation,** and whether diagonals get their own art.
- **Budget:** the model count is about (species × bins × clips × poses × 8 headings). Check
  the library size and start-up load, and the panel's memory.

## Packages (sketch)

- **R1 — pipeline spike, one species (frondgrazer).**
  - Armature, the `walk` and `idle` clips, 4 key poses each, a bake to CVM2 for the adult
    bin and 8 headings.
  - The presenter plays walk and idle, with sub-voxel in-betweens.
  - Evidence: a real-time MP4 (x264 yuv444p crf 12), flat and lit, model beside glyph.
  - `renderer-worker` for the presenter; the Blender side can be `opus-medium`.
- **R2 — format and library.** CVM2 in `model.rs`, the manifest, load time, memory on the
  panel, and the fallback when a clip is missing.
- **R3 — remaining founders and clips**, then the flight and predation species when P1/F1
  land.
- **R4 — retire the special cases** in `sleep.rs`/`idle.rs` that the clips cover, keeping
  the shader in-betweening.

## Interactions to watch

- **Sim scale.** The ecology audit (`design/7_Research/ecology-parameter-audit-2026-09-28.md`
  §3) may rescale animal mass. Visual size is set by `source.json`, so the models follow the
  body dimensions, not the org-units. Pace and stride are real space (1 BL/s cruise,
  `pace-in-body-lengths`), so walk clips should match the stride to the sim's step pace.
- **Shadows and AO.** These still see the unposed body (noted in the sleep and idle
  reports). A pose-aware occupancy is a later renderer package.
- **Training.** Arenas never draw, so rigging is presentation only and changes no sim or
  trainer contract.
