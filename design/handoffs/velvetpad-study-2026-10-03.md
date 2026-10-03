---
status: open
date: 2026-10-03
owner: opus-high worker, worktree `.claude/worktrees/velvet`, branch `velvet` (from `plants3`)
---

# Velvetpad niche: measure first, then propose numbers

**Read first:**
- the proposal: `design/velvetpad-niche-2026-10-03.md`;
- Astra's review: `design/7_Research/astra-velvetpad-niche-review-2026-10-03.md`. The verdict
  is revise; findings 2 and 3 are blocking.

Wrysk's intent: velvetpad wins only in a niche such as soggy, rarely dry ground, and the other
plants outcompete it everywhere else.

**This is a study package.** It produces data and a revised, numbered proposal. It changes
defaults and presets only behind knobs that default to today's behaviour, unless a value is
proven by the study and listed for review.

## Study

1. **Resolved configuration.** Dump the terrarium preset's resolved flora configuration for
   velvetpad and its competitors (bloomcrown, springturf, stonecushion, umbrellafrond,
   lanternberry): alpha, assimilation, maintenance, senescence, the water knobs, the light
   half-saturation, the propagule and overtop knobs. Give the reference break-even `L·μ` for
   each, labelled with its assumptions (Astra finding 1).
2. **A viability grid.** Single-stand fixtures, with the actual resolved preset, across:
   - available water: 0.6–1.4 in Astra's units, where 1 is field capacity;
   - terrain sky light;
   - canopy-transmitted light.

   For each species and cell, over one full world day and night, record net carbon balance,
   survival, foliage trend, and whether propagules get funded. **This replaces the reference
   arithmetic as the basis for thresholds.**
3. **Retreat and invasion,** with today's and the candidate velvetpad parameters:
   - Does an established mat on a drained, lit site decline? How fast? Do vacancies appear?
   - Can each competitor take a vacancy, and can it take an **occupied** pad (overtop)?
   - Does a mat on a wet, shaded site persist through a dry spell longer than the seed bank's
     life (6,587 s on the preset) and recover afterwards?
4. **A candidate mechanism.** From 2 and 3, choose the smallest set of changes that gives the
   niche. Candidates:
   - a water-budget niche: wilt/full/establish thresholds placed by the viability grid, with
     `establish_saturated_max`, keeping aeration relaxation;
   - if mats don't retreat from drained sites by water stress alone, an adult mechanism: for
     example a positive `overtop_per_check` for some non-woody competitors over velvetpad, or
     water-stress dieback;
   - a terrain-shelter birth gate **only if** shelter is the intended restriction. Keep spore
     landing and establishment separate (Astra finding 3).

   Change one advantage at a time and isolate which one carries dominance (finding 4).
5. **Plants-only check:**
   - seeds 1, 3, 5 and 6 × 6 h, today's parameters against the candidate, paired;
   - time series of occupancy by water and light class, births, donors, deaths by cause,
     litter, and occupation transitions (findings 5 and 7);
   - **fix the pass condition before running:** non-niche sites change occupants, and the
     niche persists and replaces itself.

## Out

- **The paired fauna balance measure** (Astra finding 6): Fable runs it after this lands with
  the bellwing fixes.
- **Changing the live presets' velvetpad values.** Propose them; don't apply them.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/velvet -b velvet plants3`. Commit with
  explicit paths only. Never merge, deploy, or restart a host. Headless only.
- **Stay in the flora crate** and its diagnostic examples.
- **Desktop:** at most 8 threads, `nice -n 10`.
- **Eidolon:** up to 8 threads once its soak ends (`~/cubarium-gut/ALL_DONE`). Its shell is
  fish, so run a bash script file and give your binary a new name.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

- branch and head;
- the resolved configuration table;
- the viability grid summary (where each species is viable);
- the retreat and invasion results;
- the chosen mechanism and numbers, with the evidence;
- the paired plants-only table against the pass condition.
