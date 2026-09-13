---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Fragile fauna: developmental follow-up, not a rescue result

The completed quiet cohort supplies a useful constraint on the next diversity
experiment: **skimmers are already absent in eight of the twelve mature openings**.
The remaining four populations can reproduce and some juveniles reach adult
structure, but this ten-minute follow-up cannot explain the earlier losses.
No ecological parameter or live world was changed for this analysis.

## Evidence and reproduction

Source: `captures/astra-quiet-a89179a`, frozen build `0.1.0+a89179a`, executable
SHA256 `cf82eecc00d5b7702cc3c2c01f9638cb8e2e2f9d2b263a360f50a84c4d1e040d`.
All twelve prescribed mature openings, ticks144000–156000, are retained under
both no-care and one Standard Feed at elapsed600, Front(32,48). This is the same
completed24-arm dataset as [the quiet diagnosis](astra-quiet-results-2026-09-13.md),
not a new or independent simulation cohort.

Run from the repository root:

```sh
node scripts/reduce-fauna-development.test.mjs
node scripts/reduce-fauna-development.mjs captures/astra-quiet-a89179a
```

The read-only reducer verifies the frozen executable, fixed cohort/arm coverage,
completed result flags, build and closing-hash agreement, full generation-bearing
IDs, birth/death timing and census consistency. It emits SHA256 fingerprints of
each result/event artifact and retains all eight form slots, including zeros.
It does not replace the source experiment's numerical inventory audit. Both
reducer tests and the actual24-arm reduction passed on2026-09-13; tests exercise
slot reuse, censoring, zero-exposure ratios and malformed/missing event rejection.

## Every skimmer case

Counts below are individuals, not ancestry. Opening and closing adult counts use
structure at or above the individual's adult target; immaturity is not an age or
render-scale classification. Closing totals include surviving juveniles.

| Seed | Opening (adults) | No-care births / deaths | No-care closing (adults) | Feed births / deaths | Feed closing (adults) |
| --- | --- | --- | --- | --- | --- |
| 1 | 4 (2) | 1 / 2 | 3 (0) | 1 / 2 | 3 (0) |
| 2 | 0 (0) | 0 / 0 | 0 (0) | 0 / 0 | 0 (0) |
| 3 | 1 (0) | 0 / 0 | 1 (0) | 0 / 0 | 1 (0) |
| 4 | 0 (0) | 0 / 0 | 0 (0) | 0 / 0 | 0 (0) |
| 5 | 10 (5) | 5 / 5 | 10 (5) | 6 / 7 | 9 (4) |
| 6 | 0 (0) | 0 / 0 | 0 (0) | 0 / 0 | 0 (0) |
| 7 | 0 (0) | 0 / 0 | 0 (0) | 0 / 0 | 0 (0) |
| 8 | 0 (0) | 0 / 0 | 0 (0) | 0 / 0 | 0 (0) |
| 9 | 0 (0) | 0 / 0 | 0 (0) | 0 / 0 | 0 (0) |
| 10 | 8 (4) | 3 / 2 | 9 (6) | 3 / 2 | 9 (7) |
| 11 | 0 (0) | 0 / 0 | 0 (0) | 0 / 0 | 0 (0) |
| 12 | 0 (0) | 0 / 0 | 0 (0) | 0 / 0 | 0 (0) |
| Total | 23 (11) | 9 / 9 | 23 (11) | 10 / 11 | 22 (11) |

All observed skimmer deaths were starvation. Four of twelve opening juveniles
were last observed at adult structure in each arm. Three opening juveniles died
without care, four with Feed. One new child was last observed adult without care,
two with Feed. These categories overlap and are not a survival-rate estimator.
Survivors are censored at ten minutes; last-observed size precedes a death
transaction and is not asserted to be size at death.

The pooled23→23 no-care total conceals seed1 losing both adults, seed3 retaining
only one immature animal, and seed10 gaining adults. Nor does the extra pooled
Feed birth establish improvement: seed5 has two extra deaths and one fewer adult.

## Comparison with the other forms

No-care totals and exposure fractions across all twelve worlds:

| Form | Opening → closing | Births / deaths | Immature time | Intake-flag time | Zero-reserve time |
| --- | --- | --- | --- | --- | --- |
| Grazer | 462 → 527 | 230 / 165 | 48.35% | 34.22% | 35.72% |
| Glider | 319 → 333 | 135 / 121 | 51.30% | 33.69% | 34.22% |
| Burrower | 301 → 298 | 43 / 46 | 34.31% | 84.51% | 31.28% |
| Skimmer | 23 → 23 | 9 / 9 | 56.31% | 36.17% | 36.30% |

Fractions use observed member-ticks, not equally weighted per-seed percentages.
An intake flag means any actual field intake, not its amount, nutritional value,
or specifically the manual food. Skimmer Feed fractions are56.06% immature,
36.42% intake and36.65% zero reserve. Differences are descriptive and do not
identify the causal growth, food-access or metabolic bottleneck. Forms4–7 have
zero exposure here; their ratios are null, not successful zero-percent outcomes.

## Consequence for the next experiment

Keep these mature openings for the already specified ambient-support and quiet
comparisons. However, they cannot establish prevention of early skimmer loss:
eight strata start empty. A separate developmental investigation should observe
all twelve fixed seeds from fresh ordinary worlds, or a fixed earlier cohort
before losses, without selecting surviving seeds or repopulating the live cube.

Record full IDs, parentage, actual paid births/deaths and juvenile recruitment,
plus local water/food access and actual allocation evidence sufficient to separate
food encounter, intake, growth and oxidation. Retain the empty outcomes and
compare a single mechanism only after locating the bottleneck. Do not conclude
that larger care doses, extra starting animals or cheaper offspring are warranted
from these counts. This report narrows the evidence needed; it does not discharge
the goal's diversity, unattended viability or long-run lineage requirements.

## Earlier-loss timing from retained preparation telemetry

The read-only `scripts/reduce-fauna-early-loss.mjs` now checks all twelve original
seed histories, not just their mature survivors. It verifies the frozen runner,
initial/closing snapshot header and payload identities, both manifests, all
17,280 chronological five-second telemetry samples, census/total-population
flows, zero care ledgers, and exact closing telemetry. Four focused tests cover
loss bracketing, initially absent forms, censoring/reappearance, malformed or
incomplete samples, and lossless u64 hashes. All four passed; reduction of the
real retained cohort exited zero. Reproduce with:

```sh
node scripts/reduce-fauna-early-loss.test.mjs
node scripts/reduce-fauna-early-loss.mjs captures/hunter-openings-2026-09-13
```

This is **earlier evidence from the same experiment**, not an additional cohort
or independent conservation audit. Snapshot checks here do not semantically
decode the world. Form counts cannot identify individual juvenile deaths,
food intake, or exact allocation transactions.

All twelve first samples (tick100) contain five skimmers. Their sampled peaks
are10–23, so an initial failure to produce any additional skimmers is not the
observed pattern. First net declines occur in windows ending at ticks7500–12400
(6.25–10.33 simulated minutes); these are net census changes, not first individual
death times. The complete first-loss table is:

| Seed | Peak skimmers | First zero-count bracket, ticks `(after, by]` | Count at2h |
| --- | ---: | --- | ---: |
| 1 | 13 | Not observed | 4 |
| 2 | 11 | (88700,88800] | 0 |
| 3 | 15 | Not observed | 1 |
| 4 | 11 | (94700,94800] | 0 |
| 5 | 23 | Not observed | 10 |
| 6 | 16 | (85000,85100] | 0 |
| 7 | 10 | (143700,143800] | 0 |
| 8 | 19 | (142300,142400] | 0 |
| 9 | 12 | (100600,100700] | 0 |
| 10 | 23 | Not observed | 8 |
| 11 | 11 | (103400,103500] | 0 |
| 12 | 13 | (85900,86000] | 0 |

The eight losses are first sampled at70.92–119.83 simulated minutes; none
reappear in later samples. No sampled loss of the other three initially present
forms occurs. Forms4–7 begin absent and are not counted as extinctions.

This changes the next diagnostic's scope: retain **all twelve tick-zero inputs
and follow through two hours**, with detailed early growth/parentage and later
recruitment/resource histories. A ten-minute run can inspect initial recruitment
but cannot cover these losses. Follow actual paid growth/intake/oxidation before
choosing a mechanism to change; the telemetry alone does not establish which
one causes the decline. No live reseeding or ecology change follows from this
reduction.
