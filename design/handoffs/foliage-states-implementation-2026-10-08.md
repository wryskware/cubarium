---
status: open
date: 2026-10-08
parent: foliage-variants-and-calf-round1-2026-10-07.md
---

# Foliage states: bake and draw Wrysk's picks (bloomcrown, umbrellafrond)

Shared rules: `worker-rules-2026-10-07.md`. The look is decided; implement it exactly and make no
taste calls.

## Wrysk's picks (2026-10-08)

The candidates are on branch `worktree-agent-a578908e8c16e7fa7` at babeb2a8:
`scripts/blender/foliage_variants.py` and `foliage_variants_sheets.py`. The sheets are in
`art/gen/runs/2026-10-08-foliage-variants-r2/`. Bring those scripts onto your branch (cherry-pick
or merge that branch; it is based on 5bbaf204).

- **Bloomcrown, 3 states (round-2 row 2):**
  - full: the accepted model;
  - browsed: round-1 browsed 2's stubbed rosette under a crown whose 4 lowest vanes are cut to 0.4;
  - stripped: stem, pod and bloom, plus **2 fallen pink voxels** on the ground beside the stem, at
    (+1, 0) and (−1, −1).
- **Fallen voxels must read crown pink.** Wrysk asked for "pink voxels". The round-2 candidate
  shades them dark crimson, because the colour ramp darkens low cells. Fix that, for example with a
  palette or tag override that skips the height shading.
- **Umbrellafrond, 4 states**, exactly as on the round-2 sheet:
  - full;
  - browsed A: lowest tier as light-green bare ribs, upper two tiers full;
  - browsed B: lowest tier as rib stubs, middle tier bare ribs, top tier full;
  - stripped: rib stubs on every tier.
- **States follow total foliage.** The state comes from the stand's total foliage against its total
  capacity, not from the per-layer stocks, and changes with a cross-fade, never a pop.

## Why foliage disappears from the top today (round-1 diagnosis)

- Bites take the lowest layer first (`cubarium-voxel-flora/src/lib.rs` ~5711/5732).
- Growth refills the bottom layer first (`step.rs` ~3345, `layers.rs` ~205).
- Senescence takes from the top layer first (`step.rs` ~3433, `layers.rs` ~230).
- The presenter thins each layer by its own fraction (`model.rs` ~691). So under steady browsing
  the crown drains.

## The work

1. **Bake.**
   - Bake the states into the real library (`assets/voxel-models/<mm>/`) for every crown-height step
     and every variant (phase), not only the adult step.
   - Apply the same cut rules to the smaller steps. Show a few steps in a sheet.
   - Use a per-species state count and a format version bump: a state axis per step, and a header
     table of state names and start fractions.
   - Rebake everything; no back-compatibility with old files is needed.
   - The Godot extension's reader (`crates/cubarium-godot/src/models.rs`, the legacy mesh view) must
     still build and read state 0.
2. **Select.**
   - The presenter picks the state from the stand's total foliage fraction, with hysteresis.
   - The thresholds and the hysteresis are per-species config knobs: serde fields with doc
     comments, registry rows, and a regenerated `design/parameters.md`.
   - Start values (the orchestrator's call, not Wrysk's; they can change):
     - bloomcrown: browsed below 0.66, stripped below 0.25;
     - umbrellafrond: A below 0.75, B below 0.50, stripped below 0.25;
     - hysteresis 0.05.
3. **No per-layer thinning on species with states.** The state model is drawn whole, because the
   per-layer hash is what empties the crown. Species without states keep today's thinning. Wilt and
   accents are unchanged.
4. **Cross-fade on the every-frame clock**, over a presentation knob (start at 2 s).
   - Cells present in both states stay.
   - An outgoing cell is drawn while its `keep_threshold` hash is above the blend b.
   - An incoming cell appears once its hash is below b.
   - Deterministic per stand. Moving between non-adjacent states fades directly.
5. **Leave alone:** the crown-height step dropping under dieback, the sim's layer bookkeeping, and the
   renderer work in flight (hybrid H2 owns animals in the renderer). Touch plant stamping only.

## Tests and evidence

- Tests check mechanisms on hand-built stands against the config (the state chosen at a fraction,
  hysteresis, cross-fade monotonic in b, no per-layer thinning on state species), never tuned
  numbers. Run `timeout 270 cargo nextest run -p cubarium`.
- Pictures, headless (`voxel_specimens --gpu` or `--gpu-target headless`):
  - each species at each state at 2–3 crown steps;
  - an MP4 (x264 yuv444p crf 12, real time) of one stand fading full → browsed → stripped and back.

## Return

≤40 lines: commits (branch only, explicit paths, never merge), absolute picture and MP4 paths, the
format change, and anything that looks wrong.
