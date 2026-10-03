---
status: open
date: 2026-10-03
owner: opus-medium worker, worktree `.claude/worktrees/grid`, branch `grid`
---

# The big-lever grid tool

Wrysk wants "a first-principles scan for the big levers and then doing a matrix search to find
where to start training from". This package builds the tool. Fable launches the real grid.

## Build

**A `voxel-ecology-grid` subcommand** in `cubarium-search`, beside `voxel-ecology-check`.

- **It runs a grid over master scales:**
  - **Animal metabolism `m`:** multiplies every lineage's daily need (`maintenance_per_s`,
    with motor tied to it as the search ties it) by `m`.
  - **Plant productivity `p`:** multiplies every plant species' production. That covers
    photosynthesis or fixation and growth, with nectar, fruit and seed output following.
    Find the right registry paths. If there's no single knob, scale the per-species rows
    together, list them, and say why those rows.
  - **Generic axes:** each axis is a list of `PATH=VALUE` registry overrides or a named scale.
    `m` and `p` are the defaults, so a third axis (stomach size, plant density) can be added
    later.
- **Inputs:**
  - `--m 0.5,0.71,1,1.41,2` and `--p 0.5,0.71,1,1.41,2`;
  - `--seeds`, default 8 **distinct** worlds: skip seeds the generator refuses, and record
    which;
  - `--hours`, default 2;
  - `--world <preset>`, `--threads`, `--out`.
- **Resumable:** a finished cell × seed is skipped on a rerun.
- **One thread per world** (`CUBARIUM_THREADS=1`), worlds in parallel up to `--threads`.
- **Output:**
  - `cells.jsonl`, one record per cell × seed;
  - `summary.tsv` per cell × lineage, with the median and range over seeds, for:
    - alive at 60 and 120 min;
    - births, and starvation deaths;
    - the balance measures from `live/balance.rs`: B, days to empty, surplus, R̂, access,
      sated share;
  - per plant species: stands at the start and end, and standing foliage;
  - `summary.md`: one compact matrix per lineage of median B (and R̂) over `m × p`.

## Tests

- The decode applies `m` and `p` to exactly the listed paths, and to nothing else.
- Resume skips finished cells.
- The summary of a hand-built `cells.jsonl` matches the expected medians.

## Smoke

A 2 × 2 grid, 2 seeds, 30 min, `nice -n 10`, at most 8 threads. It must run end to end and
write all three outputs.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/grid -b grid main`. Commit with explicit
  paths only. Never merge, deploy, or restart a host. Don't launch the real grid.
- **Stay in `crates/cubarium-search`** (plus an example or script if needed). Other workers are
  on flora (`plants3`) and fauna (`bellwing`).
- `design/parameters.md` is generated. On a conflict, regenerate it.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 30 lines)

- branch and head;
- the paths each axis scales, and why those;
- the CLI;
- the test results;
- the smoke output paths and a sample `summary.md`.
