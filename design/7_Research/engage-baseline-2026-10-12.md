---
date: 2026-10-12
kind: evidence
---

# Encounter baseline: today's loftstrider family against a Veilhound pack

The engagement harness (`cubarium_voxel_sim::engagement`, `config/engagement.toml`) ran
today's heuristics with no behaviour change, for package 1 of
`design/behaviour-ai-2026-10-11.md`. This is the baseline that later arms rerun on the same
starts. It is evidence, not a decision.

**Scene.**
- A bare, flat 40 × 40 m arena, in daylight.
- The pack starts 10–14 m from the family, at a seeded bearing.
- Fullness: the family 85–95 % full, the pack 10–20 %.
- Adults: 90–100 % of full size, 600–900 s old. Calves: 16–22 % of full size, 0–30 s old.
- Each episode runs at most 300 s. There are 50 seeds per arm, and the seeds are matched
  across arms.

An earlier start of 4–6 m opened every episode at contact. Three quick seeds each ended with
an adult killed in about 5 s, so the default became 10–14 m in a bigger arena.

**Result.** Job `1012-065300-engage-baseline-57b9`; per-arm table at
`art/gen/runs/engage-2026-10-12/baseline/batch-task/batch/summary.tsv`.

| arm | adult killed | calf killed | timeout | median s | pack gave up | hound wounds |
|---|---|---|---|---|---|---|
| 2 adults + calf vs 2 | 78 % | 2 % | 20 % | 41 | 0 | 0 |
| 2 adults + calf vs 3 | 94 % | 2 % | 4 % | 18 | 0 | 0 |
| 2 adults + calf vs 4 | 84 % | 8 % | 8 % | 22 | 0 | 0 |
| 2 adults vs 2 | 86 % | — | 14 % | 36 | 0 | 0 |
| 2 adults vs 3 | 96 % | — | 4 % | 17 | 0 | 0 |
| 2 adults vs 4 | 92 % | — | 8 % | 18 | 0 | 0 |
| lone calf vs 2 | — | 4 % | 96 % | 300 | 0 | 0 |
| lone calf vs 3 | — | 34 % | 66 % | 300 | 0 | 0 |
| lone calf vs 4 | — | 48 % | 52 % | 300 | 0 | 0 |

**What it says.**
- **Today the grown adult is the easy kill.** Two or more hounds kill one in 78–96 % of
  encounters, usually within 20–40 s. The design's target (topic 8) is an adult once in
  several years, with most standoffs ending in the pack giving up.
- **The calf is spared because the hounds take an adult, not because it's defended.** With
  the family present, calves die in 2–8 % of episodes. A lone calf survives five minutes
  more often than an adult does.
- **The pack never gives up, and no hound is ever hurt.** Adults have no kick, and a
  standing adult has no effect (code survey).
- **"Tests"** (a recorded break-off) are rare. Captures aren't recorded, because today's
  bites have no holding stage.

**Video.** `art/gen/runs/engage-2026-10-12/family-3-seed1.mp4` (far start, an adult killed
at 18 s) and `family-3-seed1-contact-start.mp4` (the earlier 4–6 m start).
