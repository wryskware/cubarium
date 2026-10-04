---
status: open
date: 2026-10-03
owner: opus-high worker, worktree `.claude/worktrees/assembly`, branch `assembly` (from main)
---

# Community assembly, step 1: the episode initializer and the E0/E1 starts

Wrysk said go on 2026-10-03.

**Read first:**
- `design/community-assembly-2026-10-03.md`: the proposal, then **Revision 2** (R1, R2, R6) and
  **Revision 3** (A8/N1, N2 step 1, N3). Where they differ, revision 3 wins, then 2.
- Astra's reviews: `design/7_Research/astra-community-assembly-review-2026-10-03.md`, both the
  first review and the revision-2 check.

This package is **build order step 1 only**. No source-level counters or shared budget
(step 2), no scheduler (step 3), no predator admission (step 4), and no E2.

## Deliverable

1. **The episode initializer**, one code path that every grid and search evaluation can start
   through. It:
   - loads a start directory, in the whole-world save format from save-on-demand
     (`crates/cubarium/src/voxel/save.rs`, `cubarium voxel --load DIR`);
   - checks compatibility: layer files present, tick and day phase, schema versions, the
     start's resolved parameters, build and policy identity. **An incompatible start is
     rejected, never repaired by creating or deleting matter;**
   - applies the candidate's **rate-only** overrides over the loaded state: the grid's `m`
     paths (upkeep, with motor tied) and `p` paths (`P_PATHS`). Absolute stocks, ages and
     states stay as loaded.
     - State-coupled traits stay at the start's values: stomach `k`, body scale, reserve
       capacity, tissue composition, plant capacities, lifespan, reproductive state.
     - A search gene on one of these is frozen in established runs.
   - runs derived-parameter resolution and installs the result in every layer, recording the
     start's own parameters separately from the candidate's;
   - resets controller memory, then runs a fixed burn-in that is the same for every
     candidate;
   - takes ledger baselines at the burn-in's end and at every scoring-window boundary, so
     scores are checkpoint-relative;
   - uses scoring windows fixed in advance and aligned to the day phase (N3). A failure
     during burn-in, or a start that never settles, is recorded, never excluded.
2. **Two start recipes, fixed and not candidate-specific:**
   - **E0:** today's founding, wrapped as a start. It is the disclosed early-community
     diagnostic.
   - **E1, constructed rich.** My call on the plants (revision 2 asked for "explicit
     construction"):
     - **Plants:** a plants-only grow under the preset's own default parameters to a fixed
       age, so the same fixture serves every candidate. It's conservative by construction,
       because the simulation books it.
       - The vaulttree run (`~/cubarium-gut/soak` on eidolon, old plant code) shows groves
         double every 2–3 h: 356–756 stands at 16 h.
       - Choose the age from a short measure on current main: vaulttree stands and donors,
         latticevine faces. Record it in the recipe.
       - If explicit placement of mature stands at fixed densities turns out simpler and
         cleanly booked, you may do that instead. Say which, and why.
     - **Fauna, explicit construction:**
       - roughly 3× today's consumer founders;
       - one chorister pack, sized to the prey present by a recorded rule;
       - lanternjaws as today's founding (egg bank).
       - Ages, sizes and reproductive states are drawn as consistent sets: a juvenile is
         small and not gravid.
       - All injected organic matter, mineral and energy is booked: bodies, reserves,
         stomachs, eggs.
       - Total bodies and placement attempts are capped. Failed placements are recorded,
         not silently retried into a different count.
     - **Provenance:** each start directory carries a `recipe.toml`: recipe name, seed,
       commit, config hash, plant age, fauna counts placed and failed.
3. **Paired scoring:**
   - `voxel-ecology-grid` and the search take `--start e0|e1` (or `--starts DIR`), and every
     candidate runs on the same starts.
   - **E0 stays the default** until Fable switches it.
   - Summaries say which start they used.
   - Chorister, lanternjaw and loftstrider are scored on E1 (R1), and stay readout-only on E0.
4. **A generator:** `cubarium-search voxel-starts --recipe e1 --seeds … --out DIR`, or an
   example. It is resumable, and writes one directory per seed.

## Tests (write them first)

- An incompatible start is rejected: wrong schema, missing layer, mismatched build or policy.
- Rate-only overrides change the rates and leave every stock, age and state untouched.
- Every ledger is conserved across load → override → burn-in.
- Two candidates on the same start see identical initial state: paired.
- The E1 fauna constructor books what it injects and never places a gravid juvenile.
- A failed placement is recorded and counted.

## Verify

- Generate E1 for seeds 1 and 3 on current main.
- Run a 3-cell grid (m=3 p=2, m=4 p=2, m=1 p=1) × those 2 seeds × 2 h, on E0 and on E1.
- Report per lineage: B, R̂, births, deaths by cause, and whether chorister, lanternjaw and
  loftstrider now have something to measure on E1.

Full E1 generation for 8 seeds comes after Fable reviews.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/assembly -b assembly main`.
- **Commits:** explicit paths only. Never merge, deploy, or restart a host. Headless only.
- **Scratch:** only `<scratchpad>/assembly/`.
  - `<scratchpad>` = `/tmp/claude-1000/-home-wrysk-wryskware-cubarium/0183970f-d99c-4145-9cdc-fd35b3ab6476/scratchpad`.
  - Delete nothing outside it.
- **Machines:**
  - Desktop: at most 12 threads, `nice -n 10`.
  - Eidolon: at most 4 threads (the succession study has the rest). Its shell is fish, so run
    bash script files. Use renamed binaries under `~/assembly/`.
  - No AWS.
- **Code:** `cubarium-search`, `cubarium-voxel-sim`, and the smallest hooks needed in fauna
  and flora to place and book bodies. **The succession worker owns `seeds.rs` and the flora
  overtop code:** don't touch them.
- **Plants are still changing** (the succession study). Starts are disposable and get
  regenerated when code or config changes. Don't tune anything to the current plants.
- **Processes:** never `pkill -f` a pattern in your own command line. Leave no background
  shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

- branch and head;
- the initializer's contract as built, and what it rejects;
- the E1 recipe: plant age and why, fauna counts, the pack-sizing rule, booking;
- tests, and whether each failed on main first;
- the E0 vs E1 grid table;
- open questions for Astra or Fable.
