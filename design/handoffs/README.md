---
design_status: exploration
last_reviewed: 2026-09-18
decision_refs: []
---

# Handoffs and progress

The current task index, not a design decision. Read the repository instructions,
[working policy](../../WORKING_POLICY.md), [canon rules](../0_Canon/README.md) and
any relevant [ledger](../0_Canon/DECISIONS.md) entries, then **one** task. Do not
ingest the whole research archive: reports under `7_Research/` describe their own
date and are never a current task list.

## Open now

- **Voxel performance follow-up (2026-09-18):**
  [Sparse falling water](voxel-sparse-fall-2026-09-18.md), then
  [cached exchange geometry](voxel-exchange-geometry-2026-09-18.md), on the
  same water worker; [study observer caching](voxel-study-observer-cache-2026-09-18.md)
  is separate flora/harness work. Animal sensing optimization is deferred
  while sensing is actively redesigned. These are handoffs, not landed gains.
- **2026-09-16 new-world planning handoff:**
  [Fable: voxel ringworld, engine choice and first wave](fable-voxel-world-first-wave-2026-09-16.md).
  Review the local-desktop terrain/water direction and prepare a small first wave.
  Includes the ecosystem redesign, game/product roadmap and unresolved Rust/engine
  choice. This cycle prioritizes local work; older hardware deployment instructions
  are historical context, not this handoff's scope. Apply the latest fast-iteration
  policy rather than inheriting earlier review/report workflows.
- [What a creature can sense](voxel-senses-handoff-2026-09-18.md) — Wrysk's own
  senses thread; reviews the plan himself and builds nothing until he says so.
- [Voxel art direction](voxel-art-direction-handoff-2026-09-17.md) — the look for
  the voxel organisms and strata.
- [Cheap-lane chores](cheap-lane-chores-2026-09-18.md) — bounded housekeeping with
  deterministic checks.
- [Voxel-era backlog](voxel-era-backlog-2026-09-18.md) — the open questions carried
  out of the flat-era briefs; a home for work not yet handed off.

## Landed this cycle (voxel)

One line per landed brief; the brief file and the matching `7_Research/` round note
hold the detail. These are round contracts, not open tasks.

- [First wave](voxel-first-wave-briefs-2026-09-16.md) — terrain, water and the
  first voxel rendering pass.
- [Producers](voxel-producers-briefs-2026-09-16.md) and
  [round 3](voxel-round3-briefs-2026-09-16.md) / [3b](voxel-round3b-briefs-2026-09-17.md)
  — two producer species, paid propagules, seed banks and aeration.
- [Round-4 presets](voxel-round4-presets-briefs-2026-09-17.md) — five-plant palette.
- [Round 5a transfers](voxel-round5a-transfers-briefs-2026-09-17.md) and
  [5b/5c consumers](voxel-round5bc-consumers-briefs-2026-09-17.md) — carrion,
  glowcap saprotroph, frondgrazer.
- [Render](voxel-render-briefs-2026-09-17.md), [schedule](voxel-schedule-brief-2026-09-18.md)
  — GPU cutaway and the parallel tick.
- [Replacement study](voxel-replacement-study-brief-2026-09-17.md) /
  [run](voxel-replacement-study-run-2026-09-18.md) — the study already reduced in
  `design/7_Research/voxel-round3-experiment-2026-09-16.md`.

## Archive

Older handoffs are Git history, not tasks. The 2026-09-13 flat-era briefs
(`01`–`06`) were re-homed into the [voxel-era backlog](voxel-era-backlog-2026-09-18.md)
and deleted with the dated material; old raw captures and experiment outputs were
discarded on 2026-09-13 and cannot be revalidated from local data. Source is
recoverable from `checkpoint/source-*` tags where a study recorded one. Use one
normal build cache, compact summaries and disposable outputs; Git is the archive,
not a directory of copied releases.

Later creative directions (persistent plant life history, fruiting, restrained
care rituals, selective AA, higher-resolution lighting) are tracked in the
[backlog](../backlog.md) rather than as dated briefs.
