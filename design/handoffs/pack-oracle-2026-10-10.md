---
status: open
date: 2026-10-10
owner: opus-medium, worktree `.claude/worktrees/pack-oracle`, branch `pack-oracle`
---

# PACK-ORACLE: the change-only pack disagrees with the full pack on main

Rules: `design/handoffs/worker-rules-2026-10-07.md`.
- No windows.
- No tool call over 5 minutes (`timeout 270`). A longer job runs detached with a DONE
  marker, and you hand back.
- Seed your own `target/` by reflink from `/home/wrysk/wryskware/cubarium/target`, or from
  a sibling worktree's.
- Never put worktrees or targets under /tmp.
- Commit with explicit paths.
- No `unsafe` except the approved `ash` calls in cubarium-gpu.
- Return in 40 lines or fewer.

## What is known

`crates/cubarium/examples/pack_oracle.rs` holds the change-only pack to the full pack every
tick, so its usage is in its header.
- It **passed** on GLOW-EASE rounds 1 and 2, which were built on main 55b20a39 plus the
  glow branch.
- GLOW-EASE round 3 (handback in `design/handoffs/glow-ease-2026-10-10.md`) found it
  **fails at the first tick** on main 34203675 and on its own branch. Texel (184, 21, 1),
  byte 0, is 40 in the change-only pack and 104 in the full pack: bit 0x40 differs. That
  happens on both saved states, which are now at
  `/home/wrysk/wryskware/cubarium/runs/glow-ease-states/{state-night,state-day2}`.
- Between 55b20a39 and 34203675, main merged:
  - the feeding rollout: the loftstrider round 2 rig and aim, seedporter hands
    (`own_cells`), lanternjaw, bellwing, small feeders rounds 1 and 2, and rigs with up to
    64 bones in FigureVoxel bits 14..19;
  - the vaulttree states rebake;
  - training-plan and ecology work from another session.

## Do

1. Reproduce on current main with `--state` on both saved states. Check whether the
   oracle also fails on a fresh `--seed 1 --found-ticks 6000` world.
2. Bisect the first-parent merges between 55b20a39 and main to find where it starts
   failing. Use one build per step in your worktree; incremental builds are fast.
3. Find what bit 0x40 of byte 0 means at that texel, what stands or moves there, and why
   the change-only pack misses it. Decide whether it's a real stale cell on screen or an
   oracle that is out of date.
4. Fix it at the cause. If a real cell drew stale, give the visible effect in one line.
   Add a mechanism test if one fits. The test must not pin numbers.
5. Run the oracle on both states for 1200 ticks, plus `cargo test --release -p cubarium
   -p cubarium-gpu --lib`.

Don't merge. Return:
- the bisect result;
- the cause, with file:line;
- the fix, and whether it showed on screen;
- the commits.
