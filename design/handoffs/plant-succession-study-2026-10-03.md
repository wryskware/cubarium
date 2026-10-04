---
status: open
date: 2026-10-03
owner: opus-high worker, worktree `.claude/worktrees/succession`, branch `succession` (from main)
---

# Plant succession: springturf comes up through velvetpad (study package)

**Read first:**
- the proposal: `design/plant-succession-2026-10-03.md`;
- Astra's review: `design/7_Research/astra-plant-succession-review-2026-10-03.md`. Its verdict
  is accept with changes; **its findings and its "Recommended study numbers and pass
  condition" section are this brief's spec.** Where this brief is silent, follow the review.
- background: `design/velvetpad-niche-r2-2026-10-03.md` and the study tooling
  (`crates/cubarium-voxel-flora/src/study.rs`, `crates/cubarium/examples/velvet_niche.rs`).

Wrysk, 2026-10-03: "mosses colonize first, then grasses take over, then other plants."
Velvetpad is the pioneer and springturf replaces it on open, drained ground. Woody plants are
unchanged. Velvetpad's water knobs stay as today (no C1).

## Build

1. **`overtop_covers`**, a per-species list of the ground covers its seedlings can come up
   through (review finding 2):
   - typed species names, a serde default, and a registry row;
   - an empty list means any ground cover, keeping woody behaviour; probability 0 still
     means never;
   - eligibility checks the occupant's species when the arm is taken **and again when the
     seedling sprouts**, and the final replacement is guarded by the current occupant;
   - update the comments and tests that equate "positive overtop" with "woody".
2. **Fixtures written before the code**, failing on main where they can:
   - p = 1 allowed replacement of velvetpad;
   - forbidden covers: stonecushion, springturf, and a non-cover;
   - an armed site whose occupant changes to a forbidden species;
   - gates closing between arming and sprouting;
   - seed loss;
   - woody replacement unchanged.
3. **Instrumentation** (findings 1, 3 and 7):
   - classify each turf attempt: no seed arrival, failed gate, failed arm, birth, and
     post-birth loss, with distance to the nearest turf donor;
   - count replacement events directly (velvetpad → turf, velvetpad → woody, turf → woody),
     not from minute samples;
   - track edible foliage mass, regrowth and litter input (standing litter, new shedding,
     overtopping deaths and consumption) per area and per world.
4. **Defaults: knobs only.** Springturf's `overtop_per_check` stays 0 on main defaults and the
   presets. The study sets it per arm. Propose a preset value at the end; don't apply it.

## Study

- **Arms:** springturf p = 0 (the baseline), 0.01 and 0.03, all with
  `overtop_covers = ["velvetpad"]`.
- **Unchanged:** woody 0.1, `seed_check_s` 30, arming on, runners vacant-only, and every
  turf and velvetpad physiology and dispersal knob.
- **Plants only:** seeds 1/3/5/6 × 6 h, branched from **one shared baseline hour-2 save per
  seed**. Report at 2, 4 and 6 h against S1–S3, exactly as worded in the review. **Fix the
  cohorts at hour 2, before looking at any candidate result.**
- **Fauna (S4), selected arm only:**
  - **Established:** 8 paired worlds (seeds 1/3/5/6/8/9/12/14) × 2 h, on matched-age
    established saves, baseline against the arm, with the same fauna cohort and
    introduction. Report any failed placements.
  - **Transition:** the same invaded save branched with fauna present, 2 h.
  - **Readouts:** per-seed paired B ratios; early- and late-window B, R̂, access and sated
    share; edible stocks; reproduction; reserve and body trends. Report starvation for the
    whole roster.
  - **Litter subsidy:** if a result depends on declining legacy litter from the overtop
    pulse, continue that save; don't found another long run.
- **Pre-registered next steps** (the review's last paragraph):
  - S1 fails because seeds don't arrive: dispersal or runners next, as a separate arm;
  - turf dies after entering: establishment against adult light;
  - S1 passes and S4 fails: turf production, which goes to Wrysk as a separate change.

  **Don't raise p to rescue any of the three.**

## Expected food problem

Astra finding 5: an adult turf site carries about a tenth of a pad's edible foliage
(0.054 vs 0.56 ou) and sheds about 26 % as much litter. Measure it and report it. Don't
compensate inside this package by changing turf mortality, biomass or assimilation.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/succession -b succession main`.
- **Commits:** explicit paths only. Never merge, deploy, or restart a host. Headless only.
- **Scratch:** your own subdirectory, `<scratchpad>/succession/`, only.
  - `<scratchpad>` = `/tmp/claude-1000/-home-wrysk-wryskware-cubarium/0183970f-d99c-4145-9cdc-fd35b3ab6476/scratchpad`.
  - Never delete anything outside it.
- **Desktop:** at most 8 threads, `nice -n 10`.
- **Eidolon** (`eidolon.local`, idle now): up to 20 threads.
  - Its shell is fish, so run bash script files.
  - Give binaries new names, e.g. `voxel_census.succ`.
  - Work under `~/succession/`.
  - Don't touch `~/cubarium-gut/` except to read its saves.
- **AWS:** none.
- **Processes:** never `pkill -f` a pattern in your own command line. Leave no background
  shells.
- **Files:** stay in the flora crate, its tests, and the diagnostic examples. Search and
  fauna code are read-only, except an example that reads them.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

- branch and head;
- the mechanism and fixture results (each failing on main, passing on the branch);
- the plants-only table per arm against S1–S3, with the attempt classification;
- the fauna table for the selected arm against S4, established and transition;
- the food readout: edible foliage and litter, baseline against the arm;
- the proposed preset value and what goes to Wrysk.
