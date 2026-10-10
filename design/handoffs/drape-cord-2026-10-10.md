---
status: open
date: 2026-10-10
owner: renderer-worker, worktree `.claude/worktrees/drape-cord`, branch `drape-cord`
---

# DRAPE-CORD: the vaulttree's drapes as thin continuous cords

Rules: `design/handoffs/worker-rules-2026-10-07.md` (no windows, 5-minute calls, own
`target/` seeded by reflink from main's, explicit-path commits, MP4 recipe, ≤ 40-line
return). No `unsafe` except the approved `ash` FFI calls inside `cubarium-gpu`, one call per
block. Worktrees and targets never go under `/tmp` (a RAM disk). Put every picture and video
you hand back under the **main checkout's** `art/gen/runs/drape-cord-2026-10-10/` (absolute
path), not inside the worktree. Recompile shaders with `crates/cubarium-gpu/shaders/compile.sh`
and commit the .spv.

**Another worker (GLOW-EASE, branch `glow-ease`) is editing `voxel.frag` and the emission
path at the same time.** Keep your shader changes in new functions with small call-site
edits, so the parent can merge both; don't reformat or move code you don't need to.

## Why

Wrysk (2026-10-10), after the round-crown vaulttree shipped ("trees look amazing"): "for
the drapes -- you know what i kindof wanted was like for them to have a texture that makes
them stringy and delicate, not a full voxel width. kinda like how we did a texture for
latticevine with multiple parts in the sprite sheet, itd be like that with pieces that fit
together per voxel to be like a continous cord."

Then: "i might suggest keeping the drape size a pixel constant -- like on a smaller
resolution, its still that 1-2px even though its relatively bigger then wrt/ the voxel
width."

Today the vaulttree's drape cells (bake tag `0x20|i`, `crates/cubarium/src/voxel/model.rs`;
the cyan base → tip ramp, `colours.rs` vaulttree `drape`) draw as whole cubes. The
experimental face textures (`[voxel] textures`, off by default) give drapes a cutout role
(`ROLE_DRAPE`), but that is not the default draw and is per-face, not a cord.

## What to build

- **One cord per drape column, continuous.** Each drape cell draws a cord piece entering
  at its top and leaving at its bottom. Where the cord crosses a face shared by two drape
  cells is a hash of that face's position (as the latticevine presenter hashes its shared
  edges, `scripts/blender/vine_tiles_sheet.py` docstring and the presenter it mirrors), so
  both cells agree and the pieces join. A drape cell with no drape below ends in a tip; one
  hanging from a shelf starts at the shelf. Find how the bake's strands step between cells
  (straight down, or sideways/diagonal) and keep the cord joined across every step it
  takes.
- **Constant width in screen pixels.** The cord is the same 1–2 screen pixels wide at every
  `px_per_voxel` (the desktop's 6, the Tachyon panel's, anything else), so at a small scale
  it is wider relative to the voxel. A `[voxel]`/`[light]` knob (your choice, stated) for
  the width in pixels.
- **See-through.** Off the cord, the walk carries on to whatever is behind it, as the
  latticevine's curtains do (`voxel.frag`, `VINE_CURTAIN`). The weather pass and anything
  else reading the walk's depth get the depth of what actually shows.
- **Default draw, its own switch.** Cords draw with textures off (the default), on by
  default, with a switch that gives today's cubes back. They must not depend on the
  experimental `textures` switch.
- **Keep everything else.** Today's drape colours (base → tip) and lighting; whatever
  motion drapes have today (wind sway), with the joins holding while it moves and nothing
  popping; the sim unchanged (drape cells stay cells for shade, perch and browse; this is
  draw only); both voxel sizes (0.25 m and 0.125 m libraries).
- **Mechanism: your call.** Authored cord tiles (a sheet like the latticevine's, rendered
  in `blender -b`) or a cord drawn procedurally in the shader. Pixel-constant width favours
  procedural; say which you chose and why.
- **Shadows.** Say what drape cells do in the sun march today (`world.glsl` `sunReaches`).
  A 1–2 px cord should not cast a whole cell's shadow; choose none or a thin one and say
  why.

Don't invent look beyond this: no new colours, beads or knots. If a choice is a look call
(for example a cord's wiggle inside its cell), show the options instead of picking.

## Tests (write these first; mechanisms, no tuned numbers)

- Two stacked drape cells put the cord at the same point on their shared face, for any
  position.
- The cord's width in screen pixels is the same at different `px_per_voxel`.
- A drape cell's pixel off the cord is see-through (the walk reaches what is behind); with
  the switch off the cell draws as a cube, as today.
If the cord is shader logic, test a CPU mirror of its function (as other shader logic here
is mirrored; find the pattern) rather than nothing.

## Evidence

Lit, headless, in the real renderer: a lone vaulttree and a grove (the `voxel_specimens`
grove tooling), day and night, at the desktop scale (6 px) and at the panel's scale, each
**today | 1 px | 2 px** side by side, plus a nearest-neighbour 2× enlargement of each so the
pixels can be seen. One short MP4 (real time) of a tree in wind, today | the cords, so the
joins can be seen holding.

Frame cost on the desktop GPU, before and after.

## Return

Commits; the mechanism and why; the shadow choice and why; picture and MP4 paths
(absolute); the new knobs and defaults; frame cost; what still looks wrong. Stop there: no
merge, no deploy.
