---
date: 2026-10-12
kind: evidence
---

# Encounter rerun: the VH-TACTICS Veilhound against today's loftstrider family

The same harness, scene and seeds as `engage-baseline-2026-10-12.md` (job
`1012-065300-engage-baseline-57b9`): 9 arms, seeds 1–50, at most 300 s. The only change is
the Veilhound's tactic, on by default on branch `vh-tactics` (commit 16d85c92). The adults
still can't kick (LS-DEFEND). This is evidence, not a decision.

**Job** `1012-093723-vh-tactics-engage-9ee3` (labd quick lane, 450 episodes, about 3 min).
Rows, per-arm JSONL and `summary.tsv` are in
`art/gen/runs/vh-tactics-2026-10-12/engage/batch-task/batch/`. The video is
`art/gen/runs/vh-tactics-2026-10-12/family-3-seed1.mp4`: an adult killed at 22 s, as in the
baseline's seed 1.

Each cell is baseline → rerun, in % of 50 episodes. Tests, commits and give-ups are summed
per hound over 50 episodes.

| arm | adult killed | calf killed | gave up | timeout | median s | tests | commits | give-ups | chases on calf / adult |
|---|---|---|---|---|---|---|---|---|---|
| family vs 2 | 78 → 18 | 2 → 0 | 0 → 82 | 20 → 0 | 41 → 35 | 100 | 100 | 83 | 0 / 60 |
| family vs 3 | 94 → 24 | 2 → 0 | 0 → 76 | 4 → 0 | 18 → 37 | 150 | 150 | 120 | 1 / 74 |
| family vs 4 | 84 → 24 | 8 → 0 | 0 → 76 | 8 → 0 | 22 → 41 | 200 | 200 | 162 | 1 / 81 |
| adults vs 2 | 86 → 18 | — | 0 → 82 | 14 → 0 | 36 → 35 | 100 | 100 | 83 | — / 60 |
| adults vs 3 | 96 → 24 | — | 0 → 76 | 4 → 0 | 16 → 37 | 150 | 150 | 120 | — / 74 |
| adults vs 4 | 92 → 24 | — | 0 → 76 | 8 → 0 | 18 → 41 | 200 | 200 | 162 | — / 81 |
| lone calf vs 2 | — | 4 → 0 | 0 → 100 | 96 → 0 | 300 → 80 | 0 | 0 | 100 | 50 / — |
| lone calf vs 3 | — | 34 → 0 | 0 → 100 | 66 → 0 | 300 → 80 | 0 | 0 | 150 | 50 / — |
| lone calf vs 4 | — | 48 → 4 | 0 → 96 | 52 → 0 | 300 → 80 | 0 | 0 | 194 | 50 / — |

No hound was wounded in either run. 0–11 lunges per arm met a body before their target.

**What it says.**
- **Most encounters now end in a give-up** (76–100 %), and adult kills fall from 78–96 % to
  18–24 %.
- **The give-ups come from stalling, not from the choice.** In 20 sampled episodes (family
  vs 3 and lone calf vs 3, seeds 1–10), each pack's last chase-end was `Late` (20 s with no
  closing, `give_up_stall_s`) or `Spent` (winded). None was `BrokeOff` (value below zero).
  The new `ends` readout on the row counts these.
- **The test never breaks off.** Every hound tests the family, and every test commits. It
  commits to an adult, not to the calf: the calf is chased in 0–1 of 50 family episodes. The
  family and adults-only arms have identical outcomes, so the calf changes nothing.
- **Why adults win, with the placeholder knobs.** Meat goes as length cubed
  (`meat_exp = 3`): a grown loftstrider is 3.5–3.9 meals to a hound, a calf 0.6–0.85.
  `p_kill` favours the calf by its youth and size and penalises it for its guards. But the
  risk term is small (0.3 expected kicks at half a meal each) and `commit_value` is 0, so the
  adult's value stays above the calf's and above zero.
  - Moving the choice toward calves and break-offs is a knob decision, left open here:
    `meat_exp`, `size_exp`, `p_one`, `kick_chance` with `wound_cost`, and `commit_value`.
  - LS-DEFEND's guard stance and kick also feed `guard_factor` and the risk term.
- **The lone calf is no longer killed** (34–48 % → 0–4 %). Chases stall at about 80 s, where
  the baseline pack chased for the whole 300 s.
