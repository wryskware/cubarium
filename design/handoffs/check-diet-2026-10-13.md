---
status: open
date: 2026-10-13
owner: Opus (medium), worktree `.claude/worktrees/check-diet`, branch `check-diet`
---

# CHECK-DIET: training checks read diet share and prey numbers (topic 6 watch)

## Rules

Read `design/handoffs/behaviour-ai-worker-rules-2026-10-12.md` first (the 2026-10-12 rules hold).
- Never wrap `scripts/lab/lab` in `timeout`. Quick lane only (`quick = true`,
  `hosts = ["desktop"]`, `cwd` set, under 15 minutes). The desktop is busy with training; quick
  jobs queue. Anything longer: submit and hand back with the job id. Never wait on a job; no
  tool call over 5 minutes.
- Build in your own worktree's `target/` (reflink-seeded from the main checkout's) at -j 8 or lower.
- Tests never pin numbers (no tuned values, ratios or verdicts). Every new number is a serde
  knob with a registry row (there should be none here: this is a readout).

## Why

Wrysk's worry on topic 6 (`design/behaviour-ai-2026-10-11.md` §7, "Prey switching"): a brain
scored on 10-minute energy could overeat easy prey and crash it. Decided: **every training check
reads, brains beside the heuristic, the trained lineage's diet share per prey lineage and each
prey lineage's numbers over time.** Today's check (`crates/cubarium-search/src/es/voxel/two_layer/check.rs`,
`CheckReport` ~:85, `check` ~:208, `summary_line` ~:298; `live_arm`) reports only the trained
lineage's energy, deaths and holds. Live runs write it to `run/checks.jsonl`.

## Do

1. In each live check arm (heuristic and brain, same seeds), record:
   - **diet:** what the trained lineage ate, per food lineage (animal prey by lineage, plus plant
     food as one or more buckets if the lineage eats plants): kills and energy eaten. Find where
     kills/eating are already counted (census pack-kill columns, `HarmTally`, carcass eating in
     `cubarium-voxel-fauna`) and reuse that; don't add a parallel ledger if one exists.
   - **numbers:** every animal lineage's live body count at the arm's start and end (and a few
     points between if cheap, e.g. each minute).
2. `CheckReport` gains per-arm diet shares (by kills and by energy) and prey numbers, brain beside
   heuristic; `summary_line` prints a compact diet line. Serde fields with doc comments; old
   readers ignore them.
3. Tests (mechanism, small fixtures): an arm where the trained lineage eats a prey shows that prey
   in its diet with a share in (0, 1]; shares sum to 1 when anything was eaten; counts at start
   equal the founded world's. Keep `a_plain_lineage_trains_exactly_as_today` passing (training
   path untouched).
4. Quick-lane smoke: one `voxel-live-train` run with a tiny budget (`--generations 1`,
   `--check-every 1`, short `--check-seconds`) on the Veilhound or lanternjaw if a centre exists
   (`runs/lab/*brest-clone*/run/run/*-two-layer.json`), showing the new fields in checks.jsonl.

## Boundaries

- Readout only: no change to training, scores, behaviour, knob defaults. No save/schema change.
- Don't merge (the parent merges). EC2 never. Commit with explicit paths, never -a. Trailer:
  `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.
- Never write "chorister" except as a code identifier.

## Return (at most 40 lines)

Where diet/kills were counted (file:line) and what you reused; the new fields; tests; the smoke
job id and a sample checks.jsonl line; commits; departures with why.
