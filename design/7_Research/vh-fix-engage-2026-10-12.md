---
date: 2026-10-12
kind: evidence
---

# Encounter rerun: VH-FIX (lone calves catchable, usable meals) beside the baseline and VH-TACTICS

The same harness, scene and seeds as `engage-baseline-2026-10-12.md` and
`vh-tactics-engage-2026-10-12.md`: 9 arms, seeds 1–50, at most 300 s, `FaunaConfig::default()`.
Branch `vh-fix` at 1bb31ed3 (brief `design/handoffs/vh-fix-2026-10-12.md`). The adults still
can't kick (LS-DEFEND). This is evidence, not a decision.

**Job** `1012-101243-vh-fix-engage-dd26` (labd quick lane, 450 episodes). Rows, per-arm JSONL
and `summary.tsv`: `art/gen/runs/vh-fix-2026-10-12/engage/batch-task/batch/`. Video:
`art/gen/runs/vh-fix-2026-10-12/family-3-seed1.mp4`. Traces (`engage_trace`, one line a
second per body): `art/gen/runs/vh-fix-2026-10-12/traces/`.

## Why lone calves escaped (step 1)

Traces of calf-3 seeds 1–3 on the VH-TACTICS build (`traces/before-calf-3-seed*.txt`):
- The calf flees at **2.20 m/s**, the loftstrider's adult cruise. Cruise is BL/s × the adult
  length (`manifest.rs:1225`, used at `body.rs:3129`), so a calf runs as fast as an adult.
  The code survey's "about 1.0 m/s newborn" is not what the code does.
- The hounds sprint in at 3.5 m/s and **strike as soon as the calf touches the far end of the
  capsule** (2.0–2.3 m centre to centre). The lunge covers its reach, 0.84 m, in windup plus
  lunge, 0.4 s: 2.1 m/s. Against a body fleeing at 2.2 m/s down its line it always ends
  short. The calf's wound stays 0.00 through every strike (seed 1, hound 5: windups or lunges
  at 22, 26, 35, 39, 52 and 56 s).
- Each miss costs a 3 s recovery at half speed, 1.75 m/s. That is slower than the calf, so
  the hound falls back to 3 m, sprints in again and misses again at the edge. Its stamina
  runs 0 → 1 by 64 s, and 20 s later it gives up as `Late`. Seed 1 ends at 80.6 s.
- With strikes withheld until they land, a second cause shows (`traces/timed-only-calf-2-seed5.txt`).
  The last metres' approach slows as it nears its stand, `forward = (d − stand) / step`. So
  the hound trails a fleeing body at the distance where that pace equals the body's: 1.7 m,
  `forward 1.571` (2.2 m/s), `feed 0`, from 10 s to 44 s, then `Late`.

**The fix** has two mechanisms, each a switch knob:
- `time_strikes`: strike only when a lunge begun now lands on the target as the hound
  senses it moving.
- `lead_close`: the approach adds the target's sensed pace away from the hound.

After the fix, 18 of 18 traced lone calves are killed, in 9–17 s
(`traces/after-calf-3-seed1.txt`).

## Usable meals (step 2)

`meat = min(meat by mass, usable meals × hounds at hand)`. Values from the hound's readout
(`traces/values-family-3-seed1.txt`, default config):
- A pack 10–20 % full can use **2.0 meals a hound**, 6.1 for three.
- A grown loftstrider is 1.8–2.0 hound bodies of organic matter, and a calf 0.33.

So in this scene the bound does not bind. Once the family flees, nobody faces the pack, and
an adult is worth `1.8 × 0.41 − 0.12 − 0.17 ≈ +0.5`. The calf is worth
`0.33 × 0.87 − 0.13 − 0.17 ≈ 0`: its fleeing guards still count as kickers.

Under the wide world's physiology the bound does bind:
- the Veilhound digests carrion in 1286 s;
- a grown loftstrider is 3.85 hound bodies;
- newborns are 0.6 of `body_max`;
- a hungry hound uses about 0.73 meals.

## Result

Each cell is baseline → VH-TACTICS → VH-FIX, in % of 50 episodes. Tests, commits and give-ups
are VH-FIX's, summed per hound over 50 episodes. `ends` is the pack's ChaseEnd events on the
family, by reason, over 50 episodes; the earlier runs had no `ends` field.

| arm | adult killed | calf killed | gave up | timeout | median s | tests / commits / give-ups | chases on calf / adult | ends (VH-FIX) |
|---|---|---|---|---|---|---|---|---|
| family-2 | 78 → 18 → 82 | 2 → 0 → 0 | 0 → 82 → 18 | 20 → 0 → 0 | 41 → 35 → 16 | 100 / 100 / 19 | 1 / 69 | BrokeOff 4, Killed 14, Late 14 |
| family-3 | 94 → 24 → 98 | 2 → 0 → 0 | 0 → 76 → 2 | 4 → 0 → 0 | 18 → 37 → 15 | 150 / 150 / 5 | 1 / 81 | Killed 8, Late 1 |
| family-4 | 84 → 24 → 96 | 8 → 0 → 2 | 0 → 76 → 2 | 8 → 0 → 0 | 22 → 41 → 14 | 204 / 204 / 4 | 1 / 106 | Killed 17, Late 1 |
| adults-2 | 86 → 18 → 80 | — | 0 → 82 → 20 | 14 → 0 → 0 | 36 → 35 → 16 | 100 / 100 / 21 | 0 / 71 | BrokeOff 4, Killed 15, Late 16 |
| adults-3 | 96 → 24 → 96 | — | 0 → 76 → 4 | 4 → 0 → 0 | 16 → 37 → 15 | 150 / 150 / 8 | 0 / 83 | Killed 8, Late 3 |
| adults-4 | 92 → 24 → 100 | — | 0 → 76 → 0 | 8 → 0 → 0 | 18 → 41 → 14 | 200 / 200 / 0 | 0 / 109 | Killed 14 |
| lone calf vs 2 | — | 4 → 0 → 100 | 0 → 100 → 0 | 96 → 0 → 0 | 300 → 80 → 15 | 0 / 0 / 0 | 50 / 0 | Killed 48 |
| lone calf vs 3 | — | 34 → 0 → 100 | 0 → 100 → 0 | 66 → 0 → 0 | 300 → 80 → 11 | 0 / 0 / 0 | 50 / 0 | Killed 41 |
| lone calf vs 4 | — | 48 → 4 → 100 | 0 → 96 → 0 | 52 → 0 → 0 | 300 → 80 → 11 | 0 / 0 / 0 | 50 / 0 | Killed 34 |

`Killed` counts fewer than the kills: a ChaseEnd is logged only when no other hound still
hunts the body, and an episode ends at the first eaten kill. No hound was wounded or killed.
27–192 lunges per arm met another body before their target, mostly packmates closing on the
same stand.

**What it says.**
- **Lone calves are caught**, every one, in 11–15 s median. The VH-TACTICS rerun caught 0–4 %
  of them, and the baseline 4–48 % within 300 s.
- **Grown adults die again**, 80–100 %. They flee instead of standing, and they can't kick
  (LS-DEFEND), so the same timing that catches calves lands on them too.
- **The choice still goes to adults** in this scene. A starving pack can eat a whole adult
  here, so the usable-meals bound leaves the adult's meat whole. The calf is a third of a
  meal with two kickers about it.
- **Value break-offs now happen** (`BrokeOff` 4 in each 2-hound arm), beside stalls (`Late`).
- **What should move this once LS-DEFEND lands:** adults in guard stance facing the pack.
  `facing_factor`, `guard_factor` and the risk term then push the adults' values under zero,
  the test breaks off, or the exposed calf wins.
