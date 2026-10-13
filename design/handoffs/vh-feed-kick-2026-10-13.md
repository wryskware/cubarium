---
status: open
date: 2026-10-13
owner: Opus (high), worktree `.claude/worktrees/vh-feed-kick`, branch `vh-feed-kick`
---

# VH-FEED-KICK: grown hounds kicked dead while eating, after the route fix

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first (the 2026-10-12 rules hold).
- Never wrap `scripts/lab/lab` in `timeout`. Quick lane only (`quick = true`, `hosts = ["desktop"]`,
  `cwd` set, under 15 minutes). Anything longer: submit and hand back with the job id. Never wait
  on a job; no tool call over 5 minutes.
- Build in your own worktree's `target/` (reflink-seeded from the main checkout's) at -j 8 or lower.
- Never write "chorister" except where code needs the identifier. The animal is the Veilhound.

## The finding

Hunting A/B from the same m183 saves, 2 h, 8 worlds, no calves placed (specs
`runs/route-walk-specs/hunt-{before,after}-{a,b}.toml`): before = 84f76949, after = main 415b8771
(ROUTE-WALK: the live route graph links ground the motor walks; VH-KICKED 0ae0132d is in both).
- Before: 4 kicks landed, 0 hounds dead. After: 20 kicks, 5 grown hounds kicked dead, all in
  two worlds: `runs/lab/1013-111019-rw-hunt-after-a-69df/batch-task/batch/plain/s4` (ids 106, 107
  at m207) and `.../rw-hunt-after-b-7101/batch-task/batch/plain/s7` (102, 103 at m212, 104 at
  m225; one was the pregnant breeder, so no litter: pack ends at 1). Same worlds before: 0 kicks.
- `vitals/life.csv`: each dead hound had eaten carrion (`intake_carrion` 36–87) and had
  `feeding_share` 0.8–1.0: they were feeding, not hunting, when kicked. Pack kills overall
  92 -> 77. Read with `python3 -I scripts/studies/calf-place-read.py --balance <batch dirs>`.

Likely (unverified): the route fix lets the pack reach a carcass beside the loftstrider family,
and a guarding mother kicks the feeding hounds again and again while they keep eating.

## Do

1. **Diagnose** from the saves (load the m183 checkpoint the batch used for those worlds; map
   batch world index -> start dir via the batch's args/logs): for each kick and death, the
   hound's intent, what it was eating (whose carcass, where), distance to the kicking loftstrider
   and to its calf, the loftstrider's intent (guard?), kicks per hound before death, whether the
   hound moved after being kicked. Why didn't it happen before the route fix (reachability of
   that carcass?). Cite file:line.
2. **Fix bugs** (absurd behaviour): e.g. a hound that keeps feeding through repeated kicks, or a
   pack that settles to eat inside a guarding mother's kick range, when the senses show it.
   Behaviour from senses only; every new number a serde knob with a registry row; mechanism
   tests on small fixtures, no tuned numbers. Do NOT change kick damage or other balance numbers
   (kick.damage stays 0.18; the ecology thread owns defaults). No change to the route graph.
3. If it is balance, not a bug, say so plainly with the evidence and stop.
4. **Re-check** quick lane on those two worlds' saves, same window: kicks and deaths before/after.

## Boundaries

- Don't merge (the parent merges). EC2 never. Commit explicit paths, never -a. Trailer:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`. A save change bumps the fauna schema
  to the next free number; say which. Plain training byte-identical
  (`a_plain_lineage_trains_exactly_as_today`).

## Return (at most 40 lines)

Each death's chain (file:line), bug or balance, the fix and knobs, tests, re-check job ids and
numbers, commits, departures.
