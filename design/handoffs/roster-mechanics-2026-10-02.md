---
status: open
date: 2026-10-02
owner: opus-medium worker, worktree `.claude/worktrees/roster-fix`, branch `roster-fix`
---

# Roster mechanics: the owed fixes before the viability search

The six roster animals are live on main (f50adf6d and later), and all starve or drown on
the defaults. Some of that is tuning, which is the next search's job. The items below are
**mechanics**: a behaviour or a units path that is wrong at any parameter value. Fix them
before the search, so it doesn't tune around them.

Sources:
- the "Owed" list at the end of `design/handoffs/voxel-roster-2026-10-01.md`;
- the F3 and R1 measurements recorded there.

## Fixes

1. **A glide never lands in water deeper than the glider can wade.** Today the edge-water
   refusal checks only the take-off, and 10 of 32 seedporters drowned.
   - Check the landing point and the glide's end cell.
   - A glide whose end is in water deeper than the lineage's wade depth is refused,
     exactly as a refused walk.
   - The heuristic then chooses another target.
2. **A walker never walks into water deeper than its wade depth.** The loftstrider (wade
   0.85 m) drowned in 4 of 7 seeds, in 1.5 m water.
   - Find why the motor or the heuristic lets a body step past its wade depth. A current,
     or a slide off a ledge, may be part of it; report which.
   - The body stops at the edge and turns.
   - A body that is already in too-deep water (dropped in, or the water rose) heads for
     the nearest shallows. It does not stand and drown.
3. **Latticevine seeds voided from a gut can found a vine.** Today `seed_vine` founds new
   matter, so gut seeds are lost to litter. Give voided vine seeds the same seed-bank path
   that lanternberry seeds already have.
4. **A seedporter finds fruit without luck.** Today it wanders until a fruit ray falls
   inside its 2 m cone.
   - The heuristic should use what the seedporter can sense at range:
     - the fruit or lanternberry glow or scent channel, if one exists in its observation;
     - otherwise, the nearest fruiting stand within its roost reach, the way founding
       already places it.
   - Climb or glide toward it.
   - Don't add a sense channel. The sense contract is Wrysk's. If no existing channel
     carries fruit at range, say so and stop at the founding-reach rule.

**Not in this package:**
- the film chemistry and `taste_reference` units: wait for the `film-dry` branch, which is
  another session's;
- gut transit length: tuning;
- any parameter value.

## Tests (write them first, commit them failing, then implement)

Put them in the crate where each fix lives, beside the existing roster tests.

1. A seedporter gliding from a perch whose only landing cells within glide range are
   1.5 m deep water lands nowhere: the glide is refused. With a dry cell in range, it
   lands there.
2. A seedporter's glide toward dry ground across a pond lands on the dry ground. Only the
   end cell decides, not the overflown water.
3. A loftstrider walking straight at a 1.5 m pond stops at the edge within one body
   length, with its feet never deeper than its wade depth. Repeat with its heading 30° off
   the shore normal.
4. A loftstrider placed in 1.5 m water reaches water no deeper than its wade depth within
   60 s, and is alive.
5. A gut-carried latticevine seed voided on a valid vine site enters the seed bank and can
   found a vine, by the same rule as a lanternberry seed.
6. A hungry seedporter 6 m from a fruiting lanternberry, with no fruit inside its 2 m cone
   at the start, reaches the fruit and eats within 120 s, in 8 of 8 placements around the
   stand.

Then `cargo test` passes for the touched crates.

## Measure (after the tests pass)

- Use `founder_tally`, or the roster's measure recipe from the roster brief, on the desktop
  terrarium: seeds 1–8 × 60 min, `--threads 8`, `nice -n 10`.
- Compare drownings and deaths by cause per lineage against the R1/F3 numbers in the
  roster brief.
- Report the table. Don't tune.

## Constraints

- **Worktree:** `git worktree add .claude/worktrees/roster-fix -b roster-fix main`. Commit
  there with explicit paths only, and commit early. Never merge; Fable merges.
- **Shared desktop:** use `nice -n 10`, `-j 8`, at most 8 threads per sim.
- **Headless only**; never put a window on Wrysk's screen (AGENTS.md).
- **Schema:** bump the fauna or flora schema only if a saved field changes, and say so.
- Never `pkill -f` a pattern in your own command line.
- **Leave no background shells.**
- End each commit message with:
  ```
  Co-Authored-By: Claude Opus 5.5 (1M context) <noreply@anthropic.com>
  Claude-Session: https://claude.ai/code/session_01BGC2SEjYCJTk6fgfNZcrru
  ```

## Return (≤ 40 lines)

Branch and head; per fix, the root cause found and the change; test results; the measure
table; and any item stopped short, and why.
