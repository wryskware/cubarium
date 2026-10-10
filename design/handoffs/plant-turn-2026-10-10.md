---
status: open
date: 2026-10-10
owner: opus-medium, worktree `.claude/worktrees/plant-turn`, branch `plant-turn`
---

# PLANT-TURN: plants drawn turned by quarters

Rules: `design/handoffs/worker-rules-2026-10-07.md`. No `unsafe`. Worktrees and targets
never under `/tmp`. Pictures go under the main checkout's `art/gen/runs/plant-turn-2026-10-10/`.

## Why

Wrysk (2026-10-10): "we should do the work to support plants at different angles." Every
plant model today is drawn facing the same way, so stands of one species repeat. Voxel
models turn exactly by quarter turns; animals already do (`model::turn`).

## What

- Each plant stand gets a quarter turn (0–3), fixed for its life, hashed from its stand id
  the way its variant is picked. Every species, not only the vaulttree.
- The presenter draws its model turned: `model::turn` on (dx, dz) in `each_plant_cell`
  (`crates/cubarium/src/voxel/model.rs`, ~line 917) and wherever plant model offsets are
  read (VAULT-SHAPE round 2 found `stand.rs:588`, `stand.rs:865`, `stand_delta.rs:369`;
  find all, `graft callers` / `graft grep`).
- Foliage thinning stays stable: a cell's keep/hide choice uses its **unturned** offset, so
  turning never changes which cells show.
- Sway, grow-in and the plant pass's motion stay in world axes, so a turned plant sways the
  same way its neighbours do.
- A turned model must stay on its stand's anchor (the trunk foot where the old one was).
- No schema change, no sim change: this is drawing only. A `[plants] turn = true|false`
  switch only if cheap; default on.

## Tests (write these; they check mechanisms, not tuned numbers)

1. A stand's quarter is a pure function of its id: the same every frame and across a
   rebuild of the presenter.
2. A model drawn at quarter q has exactly the cells of the unturned model, each at
   `model::turn(offset, q)`; quarter 0 is the unturned model.
3. Thinning: for a fixed fill, the set of shown cells (as unturned offsets) is the same at
   every quarter.
4. The anchor cell sits at the same world site at every quarter.
5. Across many stand ids all four quarters occur.

## Evidence

A lit still (6 px and 2×) of the same live world area, before (main) | after, with
several stands of one species visible, and a short MP4 of plants swaying, after only.
Return ≤ 30 lines: commit, the places changed, test names, paths. Stop before merge.
