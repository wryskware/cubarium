---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Food-web comparison: Codex's proposal vs Fable's Stage 1

Compares `design/ecological-niches-reconsideration-2026-09-15.md` (Codex) with
`fable-independent-food-web-2026-09-15.md` (Fable, frozen at sha256
`b2b6c102…`). After Stage 1 I read `state/telemetry.jsonl` read-only to check
Codex's numbers; that also corrects one of my claims.

## What the telemetry settles

| Tick | Pop. | Grazer, glider, burrower, skimmer | Detritus energy | Producer | Nutrient |
| --- | --- | --- | --- | --- | --- |
| 100 | 28 | 10, 9, 4, 5 | 1,335 | 275 | 643 |
| 16,100 | 176 | 42, 33, 88, 13 | 182 | — | — |
| 32,500 | 81 | 33, 22, 23, 3 | 177 | 231 | 1,117 |

- Codex is right that the boom ran on the **initial litter stock**: burrowers
  went 4 → 88 while 1,335 e fell to ~175 e, then the population halved. My
  Stage 1 blamed ongoing litter fall; after tick 20k that inflow only holds a
  steady ~175 e, so fall rate is secondary.
- My "material locked in bodies" point is weak: bodies hold 76 m of ~1,630 m
  and nutrient is abundant. Producers sit at ~0.18 m/cell against `feed_min`
  0.2 because the world **starts** near 0.21 m/cell and grazing pins it at the
  refuge: Noy-Meir's grazed-down state. This strengthens my type III / refuge
  recommendation and drops the lock-up one. Neither document noticed the low
  initial seed; "depleted" is partly baseline.

## Agreements

- Paid, slow reproduction followed by fed juvenile growth; both name the
  surplus → births → starvation lag as the boom engine.
- Specialization needs a concave trade-off; the linear `diet` split is not
  one. Codex's Morris et al. 2021 and my Futuyma & Moreno agree generalists win
  under disturbance, so tests must vary resource reliability.
- Carrying capacity from sustained production, not initial stock; feces never
  recharge; predation gated behind a stable two-level community.

## Disagreements

- **Pools vs rates.** Codex adds resource classes (litter vs carrion, seeds),
  plant internal allocation, seed transport and microbial conditioning. I hold
  that intake shape, refuge, reproduction pace and litter locality explain
  every symptom, and no new pool lifts `P` off the refuge. Central divergence.
- **Efficiency by construction.** Codex rejects a fixed trophic efficiency; my
  diet-match curve is one, chosen as the smallest concave mechanism. Codex's
  mouth/gut/digestion budget is more general with more parameters. Both are
  design choices.
- **Bands.** Codex drops the depth drive for learned habitat use; I keep bands
  as spatial decoupling. Decidable only once trained policies exist.

## Unsupported assumptions, complexity, gaps

- Codex: no evidence that a litter/carrion split or seed transport yields
  niches; its aquatic sources are admittedly not calibration. Plant structure,
  seed establishment and microbial conditioning are three mechanisms ahead of a
  measured need. It has no quantitative boom account, no refuge, no spatial
  coupling analysis.
- Fable: lock-up and litter-fall over-weighted; my ~1 m/s gross growth is ~2×
  high at actual `P` (≈0.5 m/s), lowering sustainable consumers toward ~100. No
  initial-stock account, and Codex's "each resource must be readable" is a real
  requirement I omitted.

## Retain, reject, test

| Choice | Disposition | Smallest discriminating test |
| --- | --- | --- |
| Paced paid reproduction, fed juvenile growth | Retain (both) | Cohort doubling ÷ 3×3 patch recovery > 1 |
| Concave diet trade-off | Retain; curve first, budget model if it fails | `diet` histogram at 2 h: multimodal vs central |
| Type III intake / refuge, higher initial `P` | Test first (Fable) | Same seed, intake shape swapped: minimum cube-wide `P`, visible stages |
| Initial litter stock drives the boom | Test (Codex) | Initial `De` = 0: does 28 → 181 vanish? |
| Litter vs carrion | Retain via existing `ρ = De/D`; no new pool | None |
| Litter fall locality | Test (Fable) | fall 0.02 vs 0.002 /s: burrower share after first crash |
| Seed transport, plant allocation, microbes, second producer | Reject for now | Revisit only if the tests above pass and niches still fail |
| Depth-drive removal | Defer | Per-band cross-correlation with trained policies |

Pools vs rates is not forced to consensus; the initial-stock and refuge tests
decide it in two short headless runs. Both proposals remain exploration.
