---
date: 2026-10-12
kind: evidence
---

# First per-mode clones: two-layer against plain against the rules

This is the cloning tool from BRAIN-NEXT (merged 02c78cb3), for package 7 of
`design/behaviour-ai-2026-10-11.md`.

**Setup.**
- Teacher streams: 3 training worlds × 600 s, plus seven staged mode scenes on seeds 1–8,
  using the dry-run teacher's labels.
- Held out: world seed 9 and scenes 101–104.
- Fitting: 400 updates, then one closed-loop refit round of 200.
- Jobs `1012-112209-brain-next-clone-seedpor-b589` and `1012-112210-brain-next-clone-lantern-a178`;
  outputs are in `runs/lab/<job>/run/run/summary.json`.
- **Competence** is the share of held-out staged scenes where the mode achieves its purpose:
  perched in a crown, groomed the host, kept within the nursing leash, reached a stand alive,
  rested, ate the carcass, kept clear of the hound. Arms are compared on matched seeds; the
  null-pair gap was 0.

It is evidence, not a decision.

**Seedporter**

| intent | labels | intent agreement (two-layer) | action error two-layer / plain | competence rules / two-layer / plain |
|---|---|---|---|---|
| forage | 22,819 | 0.98 | 0.051 / 0.047 | — |
| stay with troop | 1,650 | 0.83 | 0.056 / 0.054 | — |
| roost | 672 | 0.97 | 0.036 / 0.048 | 0.94 / 0.19 / 0.69 |
| groom | 547 | 0.92 | 0.054 / 0.106 | 0.16 / 0.21 / 0.29 |
| follow mother | 936 | 1.00 | 0.079 / 0.086 | 0.97 / 0.95 / 0.87 |
| flee to trees | 2,208 | 0.97 | 0.073 / 0.041 | 0.69 / 0.69 / 0.69 |
| rest | 2,277 | 0.89 | 0.060 / 0.055 | 0.16 / 0.00 / 0.01 |

**Lanternjaw**

| intent | labels | intent agreement | action error two-layer / plain | competence rules / two-layer / plain |
|---|---|---|---|---|
| patrol | 16,705 | 1.00 | 0.0015 / 0.026 | — |
| feed | 1,190 | 1.00 | 0.0011 / 0.0012 | 0.13 / 0.12 / 0.12 |
| give way | 345 | 0.97 | 0.054 / 0.049 | 0.63 / 0.06 / 0.05 |
| ambush, flee, rest | 0 | — | — | — |

**What it says.**
- **Choosing the intent works.** The top layer picks the teacher's intent 83–100 % of the time
  in every mode it saw.
- **Moving is uneven.**
  - The two-layer brain is far better at patrol, and better at groom and roost by action
    error.
  - Its roost *competence* is much worse than plain's (0.19 against 0.69), so low action error
    didn't turn into reaching the crown.
  - Rest and give-way fail for both clones.
- **Neither clone beats the rules yet.** This is cloning only, with no evolution, and it is
  far short of Wrysk's bar ("otherwise it wont be worth using either"). Next:
  - more updates and data on the failing modes;
  - a look at why roosting fails for the two-layer clone (climbing is a long sequence);
  - then stage-1 evolution on staged scenes with matched copies.
- **The lanternjaw's teacher labels ambush, flee and rest as patrol.** `hunt::steer` answers
  every sample, so those modes need a finer label mapping before they can be cloned.
- **The rules' own competence is low in some scenes** (groom 0.16, rest 0.16, feed 0.13). The
  scenes or their competence tests may be stricter than the heuristic's real behaviour;
  check them before reading the clones against them.
