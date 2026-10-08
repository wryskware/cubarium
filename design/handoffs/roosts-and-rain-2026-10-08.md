---
status: open
date: 2026-10-08
---

# Seedporter night roosts, rain grounds bellwings

Shared rules: `worker-rules-2026-10-07.md`. This is design/backlog.md §12. Wrysk said go on 2026-10-08.

## Wrysk's words

- "seedporters should prefer to roost in vault trees at night, or up top mesas they can reach by
  climbing latticevine."
- "rain should ground bellwings."

## What to build

Behaviour only. It should be coarse and bug-free, because the heuristics seed the brains later and
refinement happens there. It must not teach anything wrong.

1. **Seedporter roost.**
   - When a seedporter goes to rest at night, it prefers, in order:
     1. a vaulttree crown;
     2. a mesa or ledge top it can reach by climbing a latticevine wall;
     3. today's rest spot.
   - It picks the best site reachable within a travel budget. The budget is a knob, and the reserve
     guards work the way den rest does: Veilhound den rest (merged 1915eb45) is the precedent for
     choosing shelter against the cost of the trip.
   - It wakes and leaves by today's rules. Seedporters already climb (the climb pose, 5e819d74) and
     glide down.
   - Find out how vaulttree crowns and vine walls are represented for climbing. If a seedporter
     can't stand on a crown today, say what is missing and build the smallest correct support.
2. **Rain grounds bellwings.**
   - Above a rain threshold, a flying bellwing lands. It prefers cover (`PerchKind::Underside`,
     under a crown or overhang), then any perch it can reach (ledge, crown top, vine face), then the
     ground.
   - It stays down while it rains and flies again once the rain falls below the threshold minus a
     hysteresis.
   - It can't sip while grounded.
   - Use the rain signal the weather and plants already use, the local rain at the animal if there
     is one. The threshold and hysteresis are knobs.
3. **Knobs:** every number is a serde field with a doc comment and a registry row. Regenerate
   `design/parameters.md`. Defaults are your call; state each one and its visible effect.
4. **Tests** check mechanisms on hand-built worlds against the config:
   - a seedporter at dusk with a reachable vaulttree crown roosts there;
   - with only a vine-reachable mesa, it climbs up;
   - with neither, it rests as today;
   - an out-of-budget site is skipped;
   - a bellwing lands when the rain crosses the threshold, prefers cover, and takes off after the
     hysteresis.

   Nothing steps past a few hundred ticks, and no tuned numbers are pinned.

## Evidence

- A short check, 4 seeds × 60 sim-minutes across a night and a rain spell, on the terrarium config,
  through `chrt -i 0 nice -n 19 scripts/run-study.py --slot N -- ...`. Report:
  - where seedporters spend the night (vaulttree, mesa top, other) before and after;
  - bellwing airborne fraction in rain against dry, before and after;
  - any deaths the change causes.

  If it can't finish within one 5-minute call, launch it detached with a DONE marker and hand back.
- One headless MP4 of each behaviour (x264 yuv444p crf 12, real time):
  - a seedporter climbing to its roost at dusk;
  - a bellwing landing as rain starts.

  Write the MP4s to `/home/wrysk/wryskware/cubarium/art/gen/runs/2026-10-08-roosts-rain/`.

## Return

≤40 lines:
- commits (on your branch, explicit paths, never merge);
- the knobs and defaults, with their visible effect;
- the before/after table;
- the MP4 paths;
- any deviation from Wrysk's words and why.
