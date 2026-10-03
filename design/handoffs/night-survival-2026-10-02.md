---
status: open
date: 2026-10-02
owner: opus-high worker, worktree `.claude/worktrees/night`, branch `night`
---

# Night survival: Part 1 of the one-clock proposal

Wrysk chose this on 2026-10-02: "2h day survival, yes". The design is
`design/ecology-timescales-2026-10-02.md` (revision 3), Part 1a and Part 1b. Read it,
including its Astra findings [A2]–[A5], before starting. The review is
`design/7_Research/astra-ecology-timescales-review-2026-10-02.md`.

The search will tune the values. This package builds the mechanics, sets sensible
defaults, and proves the behaviour.

## Build

1. **Plants' night budget (1b).**
   - One spendable budget per stand at night:
     `max(0, reserve − maintenance to dawn − dawn floor) / max(time to dawn, a floor)`.
   - It funds, in priority order: leaf reflush (below the existing `reflush_below`),
     nectar, fruit.
     - Each keeps its own rate and stock caps.
     - Construction respiration is charged to the same budget.
     - Material and energy books still close (no unbooked input).
   - **New knobs, as registry rows:**
     - `night_bloom_scale`, default 0.3: nectar and fruit production at night as a fraction
       of the day rate.
     - the dawn floor, as a fraction of reserve cap.
     - the minimum time to dawn.
   - **Dawn is the ecological clock's dawn.**
     - Under `cycle`, the cycle's dawn.
     - Under `off`, there is no night, so nothing changes.
     - Under `local`, the date's sunrise. That mode is still coupled; the real-time sky is
       dropped for now.
2. **Sleep (1a).**
   - Defaults:
     - `sleep_upkeep` 0.3 for the shredder, ripple-snail, capgnawer and lanternjaw;
     - `sleep_upkeep` 0.5 for the rest;
     - `wake_hunger` 0.9 for every lineage.
   - **Change `wake_hunger` only if the tests below pass.** If a test shows harm, keep 0.5,
     say why, and leave the 0.9 behaviour behind a preset value.
   - The hunter's satiated `wake_at` is separate; don't change it.
3. **Full founders.** Founders arrive with a full reserve. If the knob is missing, add it as
   a registry row (`fauna.founding…`, default 1.0) and find where "half" is set today.
4. **Readouts.** Add these to `crates/cubarium/examples/founder_tally.rs`. Leave
   `voxel_census` alone: the lanternjaw-recruitment package is editing it.
   - per lineage and per phase (day or night):
     - sleep minutes;
     - wake causes (hunger, alert, other);
     - reserve fraction at sleep onset (`f0`) and at waking;
     - deaths by phase;
   - per plant species: night spend split into reflush, nectar and fruit.

## Tests (write first, commit failing, then implement; don't weaken them)

1. **Books close.** Over one full cycle on a fixture stand, organic in = organic out +
   storage change, with the night budget active, to the existing conservation tolerance.
2. **Bounded spend.** A stand with reserve below the dawn floor spends 0 at night. A stand
   with plenty of reserve never spends beyond the budget formula, and never drops below the
   floor before dawn.
3. **Priority.** With foliage below `reflush_below`, the reflush is paid before nectar and
   fruit. With foliage above it, nectar and fruit draw on the budget at
   `night_bloom_scale` × the day rate, capped by stock.
4. **Clock off.** With the clock off, plant behaviour is identical to today.
5. **Sleep holds.** A frondgrazer founded full at dusk on a fixture with no food sleeps
   through the night without waking for hunger.
6. **Emergency wake.** A sleeper whose reserve falls to `1 − wake_hunger` wakes and feeds
   if food is within reach.
7. **First-hit exposure.** A sleeping prey hit once wakes and goes alert, and the second
   strike uses the alert awareness, not the asleep value.
8. **Founders full.** At founding, every founder's reserve equals its cap × the knob.

## Measure (after the tests pass)

- Desktop terrarium on main's live defaults, which have the curiosity ecology baked in.
- Seeds 1–8 × 2 h, `--threads 8` or fewer, `nice -n 10`, headless.
- Run main and this branch side by side.
- Report per lineage:
  - alive at 60 min and at 120 min;
  - deaths by cause and phase;
  - sleep minutes and wake causes;
  - `f0` at sleep onset.
- Don't tune anything beyond the defaults above.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/night -b night main`. Commit there with
  explicit paths only, early and often. Never merge, deploy, or restart a host.
- **Schema:** bump the flora and fauna schemas if saved config changes. Regenerate
  `design/parameters.md`.
- **Shared desktop:** use `nice -n 10` and `-j 8`. Headless only; never put a window on
  Wrysk's screen.
- Never `pkill -f` a pattern in your own command line. Leave no background shells.
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

Branch and head, the mechanics as built (with `file:line`), test results, the defaults set
(and whether `wake_hunger` 0.9 stayed), and the measure table.
