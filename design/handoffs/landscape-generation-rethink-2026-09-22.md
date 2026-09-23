---
design_status: exploration
last_reviewed: 2026-09-22
decision_refs: []
---

# Landscape generation rethink — handoff for a new thread

Wrysk wants a fresh thread at xhigh reasoning to think about landscape
generation as a whole. This note is the starting context. It is not a brief
and decides nothing.

## Wrysk's framing (2026-09-22)

- The generated terrains look **natural** (eroded relief, soil, strata, caves).
  That is a nice feature, but it is nowhere near the concept art.
- The concept art is not natural at all. It looks like a **designed home
  terrarium**. That is a desired aesthetic, but a different one. It may
  become a toggle: a separate generator or heuristics beside the natural one.
- On desktop (`default` preset) as well as the panel (`small`), the lake reads
  as an **awkward cut-out basin in front of the terraces**: a pit with cliff
  edges rather than a lake.
- Ecology problems (plants not spreading across the map, deaths on every
  seed) are probably out of scope for the terrain thread. Terrain should
  still not make them worse. Wet ground matters, see the shore below.

## Where to look

- Art direction: `design/art-direction/Cubarium_Art_Direction_v0.1.md`.
  - The terrain grammar is at lines 78–82: slopes, ledges, banks, strata,
    cavities, pools, continuous masses. The grid is not the visual grammar.
  - The chosen camera is at line 152.
  - References are in `design/art-direction/reference/` (R5 lush organic,
    R7 silhouettes, R8 tower massing, R3/R4 current camera).
  - Wrysk's concept ref: `art/gen/runs/2026-09-21-scale-audit/wrysk-concept-ref.png`.
- Plan: `design/terrain-generation-plan-2026-09-21.md`. Slices 1–3 done.
  Slice 4, the regional atlas, is parked as issue #19.
- History and every package: `design/handoffs/terrain-generation-briefs-2026-09-21.md`
  (tiers T1–T6, shore SW, perf).
  - `design/handoffs/terrain-landforms-sol-2026-09-21.md` holds Wrysk's
    earlier correction: the tiers looked like compressed stairs.
  - Wrysk preferred the broader "revision 1" forms.
  - Accessibility on foot is **not** a requirement.
- Code (crate `cubarium-voxel`): `recipe.rs` holds the presets and `Recipe`,
  including `Tiers`. `staged_terrain` in `generate.rs` runs these stages in
  order:
  1. `heightfield` (relief in metres)
  2. `erosion::erode`
  3. `stamp_terraces` (pool chain and lake bowl)
  4. `voxelise`
  5. `lake_level`
  6. `stamp_shore`, which runs on `small` only
  7. `hollows::carve` (grottos/galleries)
  8. `prepare` (the skyline visibility pass)
  9. `seat_outlet`

  After that come `outlet_and_spring` and `repair_isolated`. Seed acceptance
  lives in the host (`crates/cubarium/src/voxel/`): a lake-size gate after a
  40-tick settle; the walk check is observation only.

## Defects found on the panel's seed 16212128481079052251 (small)

They reproduce on the desk. `crates/cubarium-voxel/examples/scratch_slots.rs`
in the terrain worktree (uncommitted) prints one-voxel fins and slots and
writes a top-down map and per-row slices. Its arguments are
`<preset> <seed> <out-dir> [noshore]`.

1. **One-voxel rock wall at the lake's edge.** The shore spares the lake's
   rim column, so the old terrace height stands between the lowered bank and
   the water. There is also a comb of stray columns on the right shore. The
   shore accounts for about 38 of the 40 fins.
2. **One-voxel lake finger** running 15 rows back into the terrace. It sits
   about 15 voxels below the ground on either side. It comes from the lake
   stamp and predates the shore.
3. **One-voxel cave skylights** through the front face, with falling water in
   them. They come from `hollows::carve` and appear with or without the shore.
4. **The cliff-edged lake.** This is how it was built, not a bug: a
   rock-bedded bowl is cut into a terrace front, with sheer terraces behind it.

## Constraints that still hold

- The world is a periodic ring in x.
- The water cycle is closed, and a cavity must not drain the lake.
- `small` must fit the panel raster (160×72×24 at 0.125 m).
- Keep voxel metres at organism scale.
- A new world format bumps SCHEMA and starts fresh; old worlds are never
  migrated.
- Tests stay short, with no pinned hashes.
- Look decisions are Wrysk's.

## Open questions for the thread

- Should the terrarium look be a second generator, a recipe family, or
  authored parts (vessel edges, placed rock masses, planted beds, feature
  pools) composed by heuristics?
- What does "designed" mean in rules: symmetry, framing masses, a focal
  pool, stepped beds, a clear foreground/midground/back?
- Should the lake stop being a stamped pit? Candidates are grading it from
  the relief, letting erosion carve it, or authoring it as a feature pool
  (terrarium mode).
- Should thin features (one-voxel fins, slots, skylights) be a hard
  generator invariant for both modes?
