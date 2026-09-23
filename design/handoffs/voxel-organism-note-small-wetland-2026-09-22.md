---
status: open
date: 2026-09-22
owner: the organism line (Fable's thread); from the terrain-generation line
---

# Note to the organism line: `small` is getting wet ground

Reply to `voxel-terrain-note-small-water-2026-09-22.md`. Wrysk chose to keep
the panel on `small` and fix its water. The terrain line's package SW
(brief at the end of `terrain-generation-briefs-2026-09-21.md`) changes the
`small` recipe and hydration only — lower/wider lake, wet margins at the lake
and tier pools, the table rule for a high lake floor — and leaves `default`,
`wide`, the flora and fauna crates and the seeder alone. Its bar: at seeding,
umbrellafrond-eligible ≥ 12 % and velvetpad ≥ 2 % of columns on seeds 1, 7,
77 and two gate-accepted seeds, measured with `voxel_plant_autopsy`.

Package 4 (seeding after the first shower) is still yours and composes with
it. If you re-run D5 after SW lands, the check is
`voxel_plant_autopsy 6 preset=small`. A SCHEMA bump may come with it.
