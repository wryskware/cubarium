---
status: open
date: 2026-10-10
owner: renderer-worker, worktree `.claude/worktrees/cell-shrink`, branch `cell-shrink`
---

# CELL-SHRINK: voxels a plant loses shrink out, and ones it regains grow in

Rules: `design/handoffs/worker-rules-2026-10-07.md`. In brief:
- No windows.
- No tool call over 5 minutes (`timeout 270`). A longer capture batch runs as one detached
  chain with a DONE marker, and you hand back.
- Seed your own `target/` by reflink. Never put worktrees or targets under `/tmp`.
- Commit with explicit paths. Recompile shaders with `crates/cubarium-gpu/shaders/compile.sh`
  and commit the .spv.
- MP4s (x264 yuv444p crf 12) go to the main checkout's
  `art/gen/runs/cell-shrink-2026-10-10/`.
- `unsafe` only for the `ash` FFI calls in `cubarium-gpu`.
- Tests check mechanisms, never tuned numbers. Return in 40 lines or fewer.

Another package, RIG-STATES, is working on animal clips in its own worktree. It shouldn't
touch the plant path. If you both change a shared file, keep both changes when you merge.

## Why

Wrysk (2026-10-10), on whether the voxels a plant loses should ease out the way growth
eases in: **"yes they should shrink"**. GLOW-EASE round 3 found the case: when springturf
cures (`cure()`, `cubarium-voxel-flora/src/step.rs` ~3765), it sheds `cure_shed` (0.8) of
its foliage in one tick. The model path then thins its cells by layer fill, so voxels
vanish in one frame. The pictures are in
`art/gen/runs/glow-ease-2026-10-10/round2/residual/`. Grazing bites do the same: foliage
drops and cells go at once.

The plant pass (`[figures] plants = "models"`, the app's default) already has the
mechanism, **grow-in** (`crates/cubarium/src/voxel/plant_motion.rs`,
`cubarium-gpu/src/plants.rs` `PlantMotion`). A plant appearing, growing a model step,
changing foliage state or going changes voxel by voxel. Each voxel scales in or out from
its own centre over `[plants] grow_s`, staggered, and the whole change takes
`foliage_fade_s`. What it doesn't cover is the cells a stand **thins** by layer fill
(`stand.rs` ~466-478, "`None` thins by layer", for species without foliage states):
shedding, bites, regrowth, wilt-driven thinning, and anything else that changes which cells
a stand draws without a step or state change.

## Do

1. **Find every path** by which a drawn plant cell appears or goes outside grow-in's
   current triggers: layer-fill thinning, cure shedding, bites, partial die-back, and any
   others you find. List them with file:line.
2. **Route them through grow-in.**
   - A cell that goes shrinks out from its centre.
   - A cell that comes back grows in.
   - Use the same `grow_s` and stagger, one rule for every plant.
   - Hold a leaving cell while it shrinks, as GLOW-EASE holds a fading emitter.
   - A cell that flips back mid-shrink reverses from where it is, not from the start.
3. **The stamped tier** (`plants = "stamped"`) can't scale voxels. Say what it does today and
   leave it, unless a fade there is cheap.
4. **Cost.** Bites and shedding can touch thousands of cells in one tick: the driven
   drought in GLOW-EASE had 5,836 events at once. Measure the pack and walk cost on
   `/home/wrysk/wryskware/cubarium/runs/glow-ease-states/state-day2`, main against the
   branch. Then say what a mass event does: whether it caps, or waits for room as the
   style table does.

## Evidence

Lit, headless, real time, today | fixed:
- A driven dry spell on `state-day2`. Use the same command as GLOW-EASE round 2's
  `drought-wake` run, in `art/gen/runs/glow-ease-2026-10-10/logs/evidence2.sh`, through
  `crates/cubarium/examples/glow_capture.rs`.
- A grazer cropping a plant up close, if a capture can show one cheaply.

Measure the turf at the shed tick: pixels moving over 150 and over 60 in one frame, as
`logs3/bigstep.py` does. Show frame cost.

Commit, and don't merge. Return the paths you found, the mechanism, the evidence, the cost
and your commits.

## Checkpoint 1 (2026-10-10, dc1628c1): what the first worker found

- **Thinning already goes through grow-in.** `PlantTracker::detect` (`sink/gpu/plants.rs`
  ~162-310) covers fill thinning, the cure shed, bites, regrowth, die-back, and step and state
  changes. Leaving cells are held and reverse from their current size. On the driven cure
  (state `runs/glow-ease-states/state-day2-f77`; the old `state-day2` no longer loads at
  fauna schema 77), 3,617 cells shrink over ~2 s, and none changes size by more than 0.071 of
  a voxel per frame.
- **What still steps is their light on their neighbours.** The AO crease and the sun and
  sky shade are read from the voxel volume. A changing cell leaves the volume (or joins it)
  at the tick, while its drawn ghost is still whole (or still at size 0). In a 6× close-up
  that is 13.9k pixels moving 20–50/255 in one frame. With `cure_shed = 0` it is 0.
  Pictures and logs: `art/gen/runs/cell-shrink-2026-10-10/`.
- **Elsewhere:**
  - The main voxel pass draws logs, sprouts, litter and carrion, which still appear and go
    whole.
  - The stamped tier is left as it is: a fade there would need per-cell alpha.
  - At x8 the shrink compresses, because it runs on sim seconds. The display runs at 1×.

## Round 2: the light follows the cell's size (option b)

The parent's pick, under Wrysk's rule that nothing pops (he's asleep; he'll see it in the
morning): **fade each changing cell's contribution to the AO crease and to the shadow by its
current grow-in size.**
- A leaving cell stays in the light's view of the world until it has shrunk, its weight
  falling with its size.
- An arriving cell's weight rises with its size.

Keep the crease's look at the ends: weight 1 is today's crease, weight 0 is no cell.

- **Cost cap.** Measure the main walk and the plant pass on the day state, main vs the
  branch, interleaved. If the change adds more than about 0.2 ms per frame to the walk at 6
  px, stop and report what it costs and why, before polishing.
- **Evidence:** the same 6× close-up at the shed tick, today | fixed, and the
  over-60 / over-20 pixel counts at the shed frame. Show one natural-day stretch too.
- **Not this round:** the main-pass logs, sprouts, litter and carrion. List what they would
  take.
