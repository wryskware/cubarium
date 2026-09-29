---
status: open
date: 2026-09-28
owner: Fable (organism line); Wrysk said go 2026-09-28
follows: voxel-lifespan-defence-2026-09-25.md (live-train fast-forwarded into main at d7cce369)
---

# Browser movement: brains or world, per-species settings, herding

Browsers still park: moving share is 0.00–0.02 in every arm, with lifespan and the
grazing defence on. Three steps. Each one is committed as soon as it is done and tested,
and reported in one line with its SHA for merge. Keep working on branch `live-train`, and
merge main in whenever it moves.

## 1. Brains or world? (~1 h)

- Run the heuristic browser (the hand-written driver) in the new-defaults world and in
  the tuned world: small, still water, seeds 1–8 × 60 min, plus 2 × 6 h.
- Report it beside round 2's brains:
  - walked m/h and moving share;
  - alive at 60 min;
  - births and starved;
  - intake per body-hour.
- The question: in the defended world, does a browser that walks do better than one
  that parks?
  - If yes, the brains are the problem.
  - If no, the world still doesn't reward moving.

## 2. Per-species settings in the ecology search

- Split `yield_by_food` and `maintenance_per_s` per lineage, and lifespan too if it is
  cheap.
- The ecology file takes an optional lineage key. The old shared keys still load.
- Re-run the search: at most 3 h, with es3's targets and constraints plus one more hard
  constraint: no plant species is lost where the default keeps it.
- Then the held-out check as before.

## 3. Herding (Wrysk's idea)

A herd grazes a spot down fast, and with the defence on, the whole herd has to move on.
It also prepares for predators, since a herd dilutes predation.

- First find out whether the browser's senses already include nearby browsers: which
  inputs, and at what range. Report it.
- Prototype herding as a heuristic arm only: the heuristic browser plus cohesion toward
  nearby bodies, and a little separation. No sense or contract change.
- Run it beside the plain heuristic in the defended world, seeds 1–8 × 60 min plus
  2 × 6 h. Measure:
  - moving share and walked m/h;
  - intake;
  - births and starvation;
  - mean nearest-neighbour distance, which shows whether they actually clump.
- Say whether herding makes browsers move more, and whether it costs them food.
- Do not change the trained browser's senses. If herding helps, the sense change comes in
  the next training round, on Wrysk's go.

## Constraints

- CCD1 under `nice -n 10`; CCD0 is Wrysk's.
- Explicit-path commits with the two trailer lines.
- Tests first for any code change, each ≤ 200 ticks.
- Block in the foreground, and kill your own wait loops before reporting.

## Return (≤ 40 lines)

The answers to 1–3 with numbers, and a recommendation for the next training round.
