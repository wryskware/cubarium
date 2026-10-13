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

## Follow-up (parent, 2026-10-13 ~15:30)

Merged 29c9e247 (branch tip 4cbac9fe). Re-check (`kick-trace-read.py --list` on
`runs/lab/1013-151156-vfk-fix2-{s4-da67,s7-e54f}/run/stdout.log`): s4 2 dead -> 0; s7 3 -> 2, and
earlier (102@204.027, 104@204.089). In s7 both hounds were labelled `give_up` (keep_clear firing)
yet stayed in motor state `Resting` at d=2.12–2.15 m from ls56 between 203.86 and 204.09 (~14 s)
while 4 kicks landed: the walk-away does not move the body. Also `ls55 go_to_calf -> h104`: a
kick landing with the loftstrider's label `go_to_calf` (VH-KICKED made the aimed kick guard-only;
is that label stale at the kick tick, or a kick outside guard?). Find why the body stays (file:line:
a rest/eat posture that must stand first, a carcass hold, the motor refusing the heading, an
action the pack layer overrides later in the tick), fix it if absurd (knob + registry row for any
number, mechanism test), and re-run the s7 trace (quick lane). Work on branch `vh-feed-kick`
after `git merge main` (fast-forward). Hand back as before.

## Follow-up 2 (parent, 2026-10-13 ~15:50)

Merged 8334c4e9 (c974f73c: `kickers_seen`). Now the hound walks clear, but past `clear_lengths`
the feed branch walks it straight back to the carcass 2 m from the family, and ls55/ls56 kill
it on a return (s7 from `runs/saves/s7-m203` in this worktree: 104@204.427, 102@204.743).
Parent's call, option (a) from the last report: **a hound weighs a carcass inside a remembered
or sensed kicker's clear distance the way the hunt weighs a kick** — reuse the hunt's risk term
(`tactics.rs` ~:753: expected kicks × `wound_cost` × (1 + own wound) × (1 − `hunger_relief` ×
hunger)) against the carcass's value to it, so a sated or wounded hound waits it out at the edge
(rest/watch outside the clear distance, the way wolves wait off a carcass) and a starving one
may risk it. No new numbers if the hunt's terms cover it; otherwise knob + registry row.
Mechanism tests (fed hound waits; starving hound goes; the kicker leaves -> it eats). Re-run the
s7 trace from m203 and from m183 (quick lane) and the s4 one from m183; hand back as before.
Branch `vh-feed-kick` after `git merge main` (fast-forward).
