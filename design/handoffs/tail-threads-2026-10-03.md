---
status: open
date: 2026-10-03
owner: opus-high worker, worktree `.claude/worktrees/tailthreads`, branch `tailthreads` (from main)
---

# Search runs fill idle cores at the tail

Wrysk, 2026-10-03, watching the AWS grid: "can the runs not be written to dynamically scale
threads to fill unused processors? like the tail end is only using 13% of the instance
now."

**Today:**
- `voxel-ecology-grid` calls `cubarium_voxel::world::set_thread_override(1)`
  (`crates/cubarium-search/src/es/voxel/live/grid.rs:744`) and runs `(cell, seed)` jobs through
  `parallel(jobs, spec.threads, …)`. Each world is single-threaded for its whole run.
- Runs differ a lot in length: about 1.1 h at plant scale 0.5 and 2.8 h at 2, on a c7g.
- So once the queue empties, the last runs hold a few cores and the rest of the host sits idle
  for hours.

## Objective

While jobs are queued, each world keeps one thread. Once the queue is empty, the worlds
still running share the machine: each world's thread budget is about `cores / running`,
re-read during the run (about once a sim-minute is enough). As runs finish, the survivors
spread onto the freed cores.

## Steps

1. **Measure first.** On the desktop (at most 16 threads, `nice -n 10`), time one world:
   - terrarium preset, seed 1, the grid's `m=1 p=2` cell (a plant-heavy one), 10 sim-minutes,
     after founding;
   - at threads 1, 2, 4, 8 and 16;
   - report the speedup curve. If one world gains less than about 1.5× at 4 threads, stop
     and report: the change isn't worth building.
2. **Find where the thread count is read:**
   - `cubarium_voxel::world::thread_override` and `default_threads`;
   - `World::step`, `Fauna::step`, and the flora step;
   - `cubarium-voxel-sim`'s `SimConfig`.

   Some are read once at construction. Make the count settable per world during a run,
   without a process-global (several worlds share the process). For example, the runner
   sets the world's or sim's thread count between steps.
3. **A shared budget in the runner:**
   - running count and queue length;
   - a pure function `budget(cores, running, queued)`, unit-tested;
   - each world applies its budget at its next check.

   Apply it to the grid. Check every other caller of the same `parallel` helper or episode
   runner in `cubarium-search`. In particular, a search generation that waits for its
   slowest episode has the same tail. Apply it there too if the same code path serves it.
   List what you changed and what you left.
4. **No change to results' meaning.**
   - Thread count is execution only. Bit-identical output is not required
     (`design/0_Canon/` and memory: conservation and qualitative reproducibility only).
   - Existing search and grid tests stay green.

## Verify

- Unit test: `budget`.
- A test that a world stepped with a thread count changed mid-run still conserves.
- **The tail itself:** a small grid on the desktop with at most 16 threads, 1 seed, 0.25 h.
  - Cells of very different length: p 0.5 and 4.
  - Run once pinned (today) and once with the budget.
  - Sample CPU use every 10 s during the tail.
  - Report wall time and tail CPU use for both.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/tailthreads -b tailthreads main`.
- **Commits:** explicit paths only. Never merge, deploy, or restart a host. Headless only.
- **Scratch:** only `<scratchpad>/tailthreads/`.
  - `<scratchpad>` = `/tmp/claude-1000/-home-wrysk-wryskware-cubarium/0183970f-d99c-4145-9cdc-fd35b3ab6476/scratchpad`.
  - Delete nothing outside it.
- **Machines:**
  - Desktop: at most 16 threads, `nice -n 10`.
  - Eidolon: don't use it (the succession study is there).
  - No AWS.
- **Code:** stay in `cubarium-search`, plus the smallest needed hook in
  `cubarium-voxel`, `cubarium-voxel-sim`, flora or fauna to make the thread count
  settable per world.
- **Processes:** never `pkill -f` a pattern in your own command line. Leave no background
  shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 30 lines)

- branch and head;
- the single-world speedup curve;
- where the thread count is now set per world, and which runners use the budget;
- the pinned-against-budget tail comparison: wall time and CPU use;
- tests run;
- anything left undone.
