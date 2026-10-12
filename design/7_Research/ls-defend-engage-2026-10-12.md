---
date: 2026-10-12
kind: evidence
---

# Encounter rerun: loftstrider defence against the new Veilhound tactics

These are the same 450 staged encounters as the baseline
(`design/7_Research/engage-baseline-2026-10-12.md`): seeds 1–50 per arm, matched starts, a
flat 40 m arena, the pack starting 10–14 m out, at most 300 s. Each cell reads
**baseline → VH-FIX → LS-DEFEND**. VH-FIX is the Veilhound tactic plus strike timing and
usable meals; LS-DEFEND adds the guard stance, sector spread, one-budget kick and the bond.
Branch `ls-defend` c5be3bfb, job `1012-104112-ls-defend-engage-7cfe`. It is evidence, not a
decision.

| arm | adult killed % | calf killed % | gave up % | median s | kicks landed | hound deaths |
|---|---|---|---|---|---|---|
| 2 adults + calf vs 2 | 78 → 82 → 0 | 2 → 0 → 0 | 0 → 18 → 100 | 41 → 16 → 5 | 1 | 0 |
| 2 adults + calf vs 3 | 94 → 98 → 0 | 2 → 0 → 0 | 0 → 2 → 100 | 18 → 15 → 5 | 1 | 0 |
| 2 adults + calf vs 4 | 84 → 96 → 6 | 8 → 2 → 0 | 0 → 2 → 94 | 22 → 14 → 5 | 12 | 1 |
| 2 adults vs 2 | 86 → 80 → 12 | — | 0 → 20 → 88 | 36 → 16 → 5 | 2 | 0 |
| 2 adults vs 3 | 96 → 96 → 8 | — | 0 → 4 → 92 | 16 → 15 → 5 | 3 | 0 |
| 2 adults vs 4 | 92 → 100 → 8 | — | 0 → 0 → 92 | 18 → 14 → 6 | 19 | 1 |
| lone calf vs 2 | — | 4 → 100 → 100 | 0 | 300 → 15 → 15 | 0 | 0 |
| lone calf vs 3 | — | 34 → 100 → 100 | 0 | 300 → 11 → 11 | 0 | 0 |
| lone calf vs 4 | — | 48 → 100 → 100 | 0 | 300 → 11 → 11 | 0 | 0 |

**What it says.**
- **Wrysk's shape now holds in the staged scene.** In his words: "multiple adults together
  can generally defend themselves. lone juveniles are the most defenseless".
  - A guarded family is almost never taken: 0–6 % adult kills, no calf kills.
  - A lone calf is always caught, in 11–15 s.
- **The pack decides at the test.** Hounds read the guard stance and break off within about
  5 s, mostly before any kick (`BrokeOff`). Only packs of four press on: they take kicks, and
  one hound in each of the two four-hound arms was kicked to death.
- **Topic-8 rates are BALANCE's to bracket.** He wants a standoff every 1–2 h that usually
  ends with the pack giving up, a hound killed by kicks about once in a few years, and an
  adult once in several years.
  - The harness ends an episode at the first give-up, so it doesn't measure re-engagement
    after the hounds rest.
  - The live-world rates come from the wide-world check (job
    `1012-104255-behaviour-stack-live-8e23`).
- **Open:**
  - The hound's risk trace read `facing 0` while adults faced it; `tactics.rs` `faces()`
    may judge a heading mid-turn.
  - The family is placed 1.5 m apart (`spacing_m`), less than a loftstrider's body, so the
    bodies start overlapping.

**Video.** Today (VH-FIX) on the left and LS-DEFEND on the right, same seed:
`art/gen/runs/ls-defend-2026-10-12/family-3-seed1-today-vs-new.mp4`. In the new episode the
pack backs off after about 5 s.
