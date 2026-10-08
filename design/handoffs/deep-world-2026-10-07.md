---
status: open
date: 2026-10-07
owner: codex gpt-6.1-sol xhigh, worktree `.claude/worktrees/deep-world`, branch `deep-world` (from main)
---

# A deeper terrarium: 1.5× and 2× depth, the new ground rolling hills into river and plains

Wrysk, 2026-10-07: "how mcuh headroom does a running world have? ... i want to try some
changes to the world size. can we make it like twice as deep, with the added area being
mainly rolling hills that become water/river/plains? should also try 1.5x depth as well to
compare."

Read `design/handoffs/worker-rules-2026-10-07.md` first.

## Today

- The live worlds (`config/desktop/terrarium.toml`, `config/voidrunner/terrarium.toml`) are
  256 wide × 128 tall × 48 deep at 0.125 m, landform preset `terrarium`
  (`crates/cubarium-voxel/src/terrarium.rs`, `tidy.rs`, erosion, the river router, an
  aquifer stream into an off-centre lake). Use `graft ask` to find your way.
- Camera: elevated orthographic, 30° tilt. Landform rule: terrain never occludes terrain;
  low at the front, peaks at the far edge.

## Build

1. **Depth 72 (1.5×) and 96 (2×).** The existing massif keeps its character at the back.
   The added depth is new ground in front of it: mainly gentle rolling hills that fall into
   lowland with plains, a river and water (ponds, wetland or lake shore, as fits). The
   front stays low.
   - Water joins the existing closed cycle (lake, aquifer stream, river router). No new draw
     on the atmosphere store. A river crossing the lowland to the lake, or the lake reaching
     into the lowland: your call; state it.
   - Depth 48 keeps its character (it is the default shape).
   - Every new shape number is a preset field (serde, doc comment with unit).
2. **Configs**: `config/desktop/terrarium-d72.toml`, `terrarium-d96.toml` and voidrunner
   twins, identical to their `terrarium.toml` except depth (and anything the camera needs).
   Don't change the existing `terrarium.toml` files.
3. **Pictures**: headless lit captures with the desktop configs, 48 / 72 / 96, at founding
   and after about 30 sim-minutes (plants established). One PNG each plus one side by side.
4. **Headroom table** (Wrysk asked how much headroom a running world has). Desktop, release
   build, live water (`water_every = 1`). Step ms/tick with its world / flora / fauna split
   (the runner's `tick cost` line, or the bench example) at minutes 0–5 and 55–60 for each
   depth, plus the headless sink's render ms/frame and pack ms/tick at a 2560×1600 target.
   - Two CPU budgets: one L3 slot (8 cores / 16 threads, like the desktop live profile) and
     6 threads standing in for voidrunner's three Zen 5 cores.
   - One founding seed per depth; this is a cost table, not ecology. 1× speed = 20 ticks/s =
     a 50 ms/tick budget, so also give the max sustainable speed multiplier per row.
   - It is a detached batch; it writes the finished table (markdown) and the captures, then
     `DONE`.

## Not in scope

Ecology tuning for the new ground, changing the default configs, deploying.

## Return

Commits; the lowland/river rule you chose; where the batch writes; picture paths; what
looks wrong.
