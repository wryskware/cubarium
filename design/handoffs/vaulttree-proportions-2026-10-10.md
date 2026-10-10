---
status: open
date: 2026-10-10
owner: opus-medium, worktree `.claude/worktrees/vault-shape`, branch `vault-shape`
---

# VAULT-SHAPE: candidates for the taller vaulttree's proportions

Rules: `design/handoffs/worker-rules-2026-10-07.md` (no windows, 5-minute calls, own
`target/` seeded by reflink, explicit-path commits, ≤ 40-line return). Blender runs `-b`
only. Worktrees and targets never go under `/tmp`. Put every picture you hand back under
the **main checkout's** `art/gen/runs/vaulttree-proportions-2026-10-10/` (absolute
path), not inside the worktree.

## Why

Wrysk (2026-10-10): "the new taller vaulttrees. theyre looking a little sparse. height is
working, but proportions feel stretched. also wonder if its worth thinking about the
overlap/spacing of the stands wrt/ canopy fillage. not saying it needs to change, just
something to consider."

411fd108 raised the vaulttree's `crown_height_m` from [2.25, 3.5] to [2.25, 6.5] and left
`crown_radius_m` at [0.625, 1.0] (source.json and the flora config). So the mature tree went
from about 3.5 m tall by 2 m across to 6.5 m by 2 m: a column. The height stays (the
loftstrider and Veilhound reach rules depend on it).

## What to make (pictures only; no config, no sim, no merge)

Bake each candidate into a scratch library with `scripts/blender/bake_voxel_models.py`
(`--source` a copied source.json; the procedural `build_plant`), never over
`assets/voxel-models/`. The candidates, all at 6.5 m:

- **A. Today**: radius 1.0 m.
- **B. The old shape, scaled**: radius 1.85 m (3.7 m across, the old tree's 1.75 height to
  width).
- **C. Between**: radius 1.4 m.
- **D. Same width, fuller**: radius 1.0 m, the crown's foliage made denser (lower porosity
  or more foliage share, state which), so it reads less sparse without widening.

Keep every other rule of the builder as it is; if a candidate needs a thicker trunk to
not look absurd, say so rather than change it.

Show, in the real renderer (`voxel_specimens --gpu` or the figure/plant capture you find
fits), lit, at the desktop scale (6 px a voxel) and 2×:
1. the four mature trees side by side, with a loftstrider and a seedporter for scale;
2. a **stand**: five mature trees at the spacing mature vaulttrees actually reach in a
   fresh world (measure it: nearest mature neighbours in a short fresh run or from the
   spacing rule in `cubarium-voxel-flora`, and state the number and how you got it), once
   for A and once for B, so he sees canopy fill.

## Also report (two or three lines each)

- What `crown_radius_m` drives in the sim (shade, browse reach, seedling spacing,
  competition), so a wider crown's ecology cost is known before he picks.
- Whether the plant pass or bake has anything else that makes the tall crown look sparse
  (porosity holes, leaf cutouts, stage thresholds).

## Return

Commits (the sheet script changes, if any); absolute picture paths; the measured stand
spacing; the cell counts per candidate; the findings above. Stop: Wrysk picks.

## Round 2 (2026-10-10): B, more shelves, a grove, four sides, lean

Round 1 (branch `vault-shape`, 74055726) added `voxel_specimens --crown-radius / --scale /
--stand`, the `vault_spacing` example and the scratch `shelf_fill`; stand spacing measured
1.38 m in World 2. Build on it, same worktree and branch.

Wrysk: "start with B, but i would experiment with more shelves. will want to see it in a
canopy too cuz fewer shelves could be fine there. also, can we show it from other angles?
lets at least be able to rotate on 90deg.. not sure about the lean.. maybe just rotate the
canopies and vary lean less because in a grove trees would lean together."

- **B at three shelf counts**: radius 1.85 m with 12 (the pick's), 17 and 22 shelves, by
  round 1's gap-filling rule. Name them B12, B17, B22.
- **Four sides**: each lone tree from the four quarter turns (0°, 90°, 180°, 270°). Find
  how the sheet can turn a model by a quarter (a voxel model turns by quarters exactly) and
  add it as a flag; also report whether the live plant pass can draw a plant turned by a
  quarter today, and what it would take if not.
- **Lean**: find the vaulttree builder's lean and its variation across the three variants
  (`scripts/blender/organism_lineup.py`, near `vault_shelf`). Make one scratch rule where
  every variant leans the same way and less (state the angles before and after), the
  variety coming from turning each tree by a quarter instead.
- **Grove**: seven B trees at 1.38 m (round 1's measure) in a loose cluster, each turned by
  its own quarter, all leaning the same way under the new lean rule: once each for B12,
  B17 and B22, seen from 0° and 90°. Then the same seven with today's lean variation, B17
  only, for comparison.

Same rules and outputs as round 1 (`art/gen/runs/vaulttree-proportions-2026-10-10/` in the
main checkout, 1× and 2×, lit, with a loftstrider and seedporter for scale). Nothing in
`assets/voxel-models/` changes yet.

Return: commit; picture paths; the lean angles before and after; cells per shelf count;
whether the live pass can turn plants by quarters. Stop: Wrysk picks.
