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

## Round 3 (2026-10-10): ship B, round crown, shelves by foliage

Round 2 (9ffcc163) made B12/B17/B22, the four-side views, the shared-lean rule and groves.
Wrysk, after opening the .glb exports in Blender: "huh, didnt realize vault trees were so
thin. that seems unnatural that its much skinner from one angle. as for the variants, i
think they all have a place. maybe depending on some foliage scale, the trees can vary from
b12 to b22. and definitely keep the 0/1/2 variants of each for variety in the world. as for
lean and canopy decisions, idk what to tell you yet. we should do the work to support
plants at different angles. leaning all the same way may not actually matter tbh. we can
try first without that and then decide to enforce it if it looks off."

This round ships, on the same branch, for merge:

1. **Round crown.** Today's crown is about 2.1 × 1.5 m on main (much thinner along z). Find
   why in `vaulttree()` / `vault_shelf` (`scripts/blender/organism_lineup.py`; it was built
   for a front-view lineup) and make the crown about as wide along z as along x, keeping the
   shelf look, its count and its sizes. Measure both widths before and after.
2. **B for real.** The vaulttree's `crown_radius_m` max becomes 1.85 m in the flora config
   default (`cubarium-voxel-flora`, regenerate `design/parameters.md`), then source.json via
   `voxel_model_source`, then a rebake of both voxel sizes into `assets/voxel-models/`.
   Leave the min radius as it is unless the bake breaks; say what you did.
3. **Shelves by foliage.** My reading of "some foliage scale" (state it in the commit; Wrysk
   can correct it): the shelf count follows the tree's foliage, using the existing foliage
   states (`foliage_state_below`, as the browse states do): a thin tree draws B12, a middle
   one B17, a full one B22. Keep variants 0/1/2 for each. Say how the states interact with
   the presenter's per-cell foliage thinning, and pick the one that doesn't double-thin.
4. **Lean**: today's rule unchanged (drop the shared-lean scratch rule from the bake path or
   leave it off by default). The quarter turns come from the separate PLANT-TURN package.

Evidence: the lone-tree four-side strip and the 7-tree grove (both angles) from round 2's
tooling, rebuilt from the shipped library: before (main) | after. Plus a short lit capture
or still of a live world area with vaulttrees, if one is cheap. .glb exports of the
shipped variants with `voxel_export` (merged on main) into
`art/gen/runs/voxel-export-2026-10-10/` as `vaulttree-shipped-<state>-v<N>.glb`.

Tests: none beyond the existing suites (this is a bake and a default), but run
`cubarium-voxel-flora`, `cubarium-voxel-fauna` and `cubarium` tests (each under
`timeout 270`, detached if longer) since the radius is an ecology knob; report failures,
don't re-pin numbers. Return: commits, widths before/after, cells per state and variant,
test results, picture and .glb paths. Stop before merge.

## Round 4 (2026-10-10): two thinner states, down to bare

Round 3 merged (16684755): shelves follow foliage, b22 / b17 / b12 by
`foliage_state_below` [0.66, 0.33], variants 0/1/2. A dying vaulttree still draws 12
shelves, so the crown never thins past b12. Wrysk: "yah probably need to add like a 6L and
0-1L for continuity on the trees."

My reading (state it in the commit; he can correct it): two more foliage states below b12,
so a thinning tree keeps losing shelves down to bare:
- **b6**: 6 shelves (`shelf_fill` 6/12).
- **b0–1**: the barest. Some variants draw one shelf and others none, so a dying grove
  isn't uniform; say which variant draws which. With no shelf there is no drape.

Five states then need four thresholds in the flora config's vaulttree
`foliage_state_below` (`cubarium-voxel-flora`, regenerate `design/parameters.md`), then
source.json via `voxel_model_source`, then a rebake of both voxel sizes into
`assets/voxel-models/`. Space the thresholds evenly unless the stand's foliage range says
otherwise, and state the numbers and why. Check the presenter takes five states without
change (`stand.rs` near line 1109) and fix it if not.

Also say, without changing it, whether the juvenile bins draw the same model in every
state, and why.

Evidence: the lone tree in all five states (variants 0/1/2), and the 7-tree grove from
round 2's tooling with mixed states, from main | after. Also .glb exports of the new
states into `art/gen/runs/voxel-export-2026-10-10/` as `vaulttree-shipped-<state>-v<N>.glb`,
and add them as a row to the vaulttree .blend scene (`scripts/blender/glb_scene.py`).

Tests: none new (a bake and a default); run `cubarium-voxel-flora` and `cubarium` tests
(each under `timeout 270`, detached if longer) and report failures without re-pinning.
Return: commits, thresholds, cells per state and variant, picture and .glb paths. Stop
before merge.

## Round 4b (2026-10-10): the 2-shelf barest variant, two medium shelves

Round 4 is on branch `vault-states` (1d7a02c5, unmerged; its full rebake was stopped by
the parent before it finished, so `assets/` is not rebaked yet). Wrysk kept the 0/1/2
barest variants and asked: "can the 2S tree be like two medium shelves instead of the big
one at the top and medium one at the side? choose like a different branch for a medium or
small shelf."

Today b0-2 variant 2 keeps shelves {0, 6} of `VAULT_SHELVES` (0 is the big crowning shelf,
ph 190; 6 is medium, ph 115, right side). My pick: **{2, 6}**: shelf 2 (ph 140, left, high)
and shelf 6 (ph 115, right, mid), two medium shelves on opposite sides of the trunk. Both
are in b6's set {0, 2, 4, 6, 8, 10}, so every state still contains the thinner states'
cells. Variants 0 (none) and 1 (the crowning shelf) stay.

Make the barest state's kept shelves explicit per variant (e.g. a `shelves` list per
variant in the state's spec, picked by variant in the bake as `shelf_fill` lists are),
rather than the evenly spaced rule. Check in the picture that 2 and 6 hang from different
limbs; if they share one, pick the nearest medium pair from b6's set that doesn't, and say
which.

Then: merge main into the branch first (main has the drape cords now, so the pictures show
cords; `voxel_specimens.rs` may conflict with the drape tooling, keep both), re-run the
round-4 pictures for the barest state (lone v0/v1/v2, the barest groves, the 15-tree mixed
grove), re-export `vaulttree-shipped-b0-2-v2.glb` and rebuild the .blend, then relaunch
the full rebake and its checks detached (`runs/bake.sh` then `runs/after-bake.sh`, which
writes `runs/after-bake-DONE`) and hand back. Return as round 4.
