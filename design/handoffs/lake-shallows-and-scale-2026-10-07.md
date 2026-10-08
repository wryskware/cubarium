---
status: open
date: 2026-10-07
owner: opus-medium worker, worktree `.claude/worktrees/lake-shallows`, branch `lake-shallows` (from main)
---

# Swamp shallows back, and the big lake as one generation scalar

Wrysk, 2026-10-07, watching a terrarium after today's lake changes: "not seeing a lot of
reeds. did your lake change wipe out the shallow swamp areas? and can you make the big lake
something that we can toggle in the generation settings. or even a scalar parameter?"

Read `design/handoffs/worker-rules-2026-10-07.md` first.

## Today's lake changes (suspects)

- cd481545 (lake-cave, brief `lake-cave-2026-10-07.md`): `lake_bed` became a wandering
  outline whose bed shoals as `depth × (1 − u²)`, and banks within `lake_bank_m` are
  graded to water + 1 + `lake_bank_slope` × distance. Grading the banks a voxel above the
  water may have removed flat wet margins at water level.
- eb8e82d5 (wide-lake, brief `wide-lake-2026-10-07.md`): in the wide config only,
  `water_level_m` 0.375 → 0.625, `lake_depth_m` 0.75, `lake_m` [6,8], `lake_bay` 0.75,
  `lake_clear_of_seam` on. Wrysk has since said lake position doesn't matter (he is adding
  panning), so turn `lake_clear_of_seam` back off in the wide config.

## 1. Find out

Siphonreed (and any other wetland plant) needs shallow water or saturated ground; use
`graft ask` to find its habitat rule. Measure **shallow-water area** (standing water above
ground no deeper than the reed's limit), **saturated-ground area** and **siphonreed
stands**, at founding and at 30 sim-minutes, on seed 1 and 2:
- d96 at f9f9c132 (before the lake changes) against main;
- wide at a9b52dba (before the bigger lake) against main.

Use detached worktrees of those commits (seed their `target/` by reflink from yours).
Report what took the shallows away.

## 2. Fix

Bring the swamp back: a shallow, marshy margin round the lake (a shelf at the reed's
depths, wet flats at water level), as wide as the old one or wider, without returning to
straight edges. New shape numbers are preset fields (serde, doc comment with unit).

## 3. The big lake as one scalar

Add one generation scalar, for example `lake_scale` under `[world.landform]`: 1.0 is the
standard lake. Larger values grow the lake (length, width and bay, depth up to the bedrock
limit) and its shallows. Say whether it also raises the water level, and why. The wide
config uses only this scalar (pick the value that matches today's big lake) instead of its
separate lake overrides. Every existing field stays individually overridable. It is a world
generation setting that the new-world screen will expose later.

## Pictures

2560×720 for the wide world and the usual 3840×832 strip for d96: before and after at 30
minutes, plus a crop of the lake margin. Also the wide world at three `lake_scale` values.

## Return

Commits; the cause; shallow area and reed stands before, at the regression and after; the
scalar's definition; picture paths.
