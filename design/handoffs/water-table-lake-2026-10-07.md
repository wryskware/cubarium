---
status: open
date: 2026-10-07
owner: opus-high worker, worktree `.claude/worktrees/water-table`, branch `water-table` (from main)
---

# The big lake is a raised water table over natural ground, not a dug-out

Wrysk asked for the wide world's lake to grow by **raising the water table** ("id maybe raise
the water table? just that proportionally, i think theres more room for lake"). Then: "can
you make the big lake something that we can toggle in the generation settings. or even a
scalar parameter?"

What was merged instead (b58c5853, `lake_scale`) enlarges and deepens a carved basin and
puts the table back at 0.375 m. His verdict on the pictures
(`/tmp/claude-1000/-home-wrysk-wryskware-cubarium/00bac523-e74c-4d3f-8e6c-2a36c4f9fafa/scratchpad/lake-batch/out/pictures/wide-lake-scale-1.0-1.5-2.5.png`):

> "you just created digouts. i told you to raise the water table. this lake scale is not
> usable. i mean its OK for there to be some places naturally where theres not a shor and
> has a small cliff, but this is extreme and looks like you just took a cookie cutter slice
> out the world and didnt even smooth it with erosion"

Read `design/handoffs/worker-rules-2026-10-07.md` first. Then read the generator's lake path:
`layout`, `lake_bed`, bank grading, the shelf and flats, `lay_lake`
(`crates/cubarium-voxel/src/terrarium.rs`), and how `water_level_m` floods "everything open
below this line". Use graft.

## Build

1. **The lake's size comes from the water table.** The scalar Wrysk wants is the water
   level: raise it and water fills the ground's natural low places. The shore is wherever
   the water meets the terrain, so it follows the contours. `water_level_m` (or a clearer
   name if you rename it) is the generation setting; document its visible effect.
2. **No carved slices.** The basin is shaped into the heightfield before erosion, as a smooth
   depression with wide, gentle sides, so erosion finishes it like the rest of the land.
   Remove the vertical cuts: the straight back wall, and banks graded as a separate pass
   after erosion. An occasional short natural drop at the shore is fine; long straight
   walls are not.
3. **Shallows come from the slopes.** Gentle ground at the waterline gives reed water
   (siphonreed stands in 0.1–0.5 m) and wet flats naturally. Keep or drop the shelf and
   flats code as the new basin needs; reeds must not regress (the margin example
   `crates/cubarium/examples/lake_margin.rs` measures shallow, deep and saturated area and
   reed stands).
4. **`lake_scale`**: remove it, or keep it only as the basin's footprint if that still earns
   its place with the table doing the work. Say which and why. The wide config uses a
   raised water table.
5. **Every depth looks right at its default**: d48 (now mostly marsh: get its open water
   back), d72, d96 and wide. Report grove sites, since a higher table rules out low root
   sites.

## Pictures

Founding and 30 sim-minutes:
- the wide world at three water-table values, 2560×720, stacked;
- d48 and d96 at their defaults.

Include a close crop of a shore, looking for straight walls.

## Return

Commits; what you removed; the scalar and its values; shallow area and reed stands; grove
sites; picture paths.
