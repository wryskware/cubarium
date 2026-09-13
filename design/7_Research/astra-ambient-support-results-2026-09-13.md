---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Ambient rainfall: complete two-hour screen

Retention update, 2026-09-13: raw captures and frozen worktrees cited below were
deleted at Wrysk's request. The subsequent 24-hour collection finished, but was
not fully interpreted before deletion. This report establishes no 24-hour result.
Use the [current handoff](../handoffs/06-autonomy.md), not old launch instructions.

**Disposition:** the isolated rainfall experiment is technically consistent. It
does not establish that 90% rainfall improves unattended ecology or that care is
needed. Keep the production default unchanged. The same frozen recipe is eligible
for the proposed longer-horizon evidence, not promotion to a default.

## Evidence and strict reduction

Source is `3e9bc2f`, frozen executable SHA-256
`17b624087fb51874f6abe523860abec5cbfb47854ff917a0ca23804c6fc91af0`, in
`captures/ambient-rain-two-hour-3e9bc2f`. Parent confirmed the existing process
terminated with exit 0; this review did not restart it or run new simulation.
All twelve prescribed schema-9, tick-144000 openings were retained, totaling
1,105 opening organisms. Each of six arms ran 144,000 further ticks, ending at
288,000: 72 complete arms and 51,840 ten-second samples.

[Reducer](../../scripts/reduce-ambient-support.mjs) and
[tests](../../scripts/reduce-ambient-support.test.mjs) are read-only tools.
Run `node scripts/reduce-ambient-support.test.mjs` (10 tests passed) and
`node scripts/reduce-ambient-support.mjs captures/ambient-rain-two-hour-3e9bc2f`
(exit 0). The [reduction artifact](assets/astra-ambient-support-reduction-2026-09-13.json)
retains all 72 arm summaries, input and endpoint identities, eight forms including
zeros, ancestry, all three local regions, receipts and matched contrasts; it is
not a copy of the raw sample streams.
The six existing snapshot/preparation tests also passed during this review.

The reducer checks pinned executable identity, original snapshot envelopes and
hashes, all twelve seeds, exact two-hour cadence/endpoints, all six arm identities,
common PRE inventories, only the natural rain-rate config difference, untouched
100% rate/state, exact `opening_rate * 0.9`, and independent doses. It streams
each sample file, reconciles population with window births/deaths, bounds local
cell-time counts, checks water accounting and manual attribution, validates every
receipt, and checks closing snapshot envelopes/hashes. Missing, failed, malformed
or incomplete cases refuse the full pass and remain represented in output. Tests
include strict-limit equality, nonfinite values, shifted/missing/extra samples,
bad receipts, zero populations, endpoint mismatches, factor contamination and
retention of a failed seed. The tool never writes into the run directory.

Recorded material, water, corrected persisted energy, independent window energy
and receipt-boundary energy peaks all remain below the unchanged per-seed
`1e-8 * max(PRE inventory, 1)` limits. Across 72 arms their maximum absolute
values are respectively `1.74623e-10`, `6.12999e-10`, `6.28688e-11`,
`4.94765e-10`, and `0`. All legacy raw audits also passed here, maximum raw
energy drift `1.49516e-6`; the reducer would retain a raw failure separately.
This checks recorded energy/material gates, not an independent semantic snapshot
decoder or transaction replay. Tiny signed natural-attribution subtraction noise
is retained and bounded by the original water limit, never clamped to zero.

The [prior harness review](astra-ambient-harness-review-2026-09-13.md) records
16 ambient and 9 care tests, including no-input observation identity, full-state
single-field isolation, independent factors, and Generous mid-shower restart at
90%. It also records complete `care_compare` stdout identity against frozen
`4d10351` at 2400 ticks for seed 1/default and seed 3/Generous isolated Rain.
Together with this run's untouched reference openings, those satisfy the proposed
default-identity gate for extending the unchanged experiment. They do not assert
that every two-hour trajectory was rerun against the old executable. Experiment
output resume is intentionally unsupported. This reducer itself is pinned to the
two-hour contract; a later reducer must explicitly validate the longer horizon.

The 48 cared arms each made 60 attempts: all 2,880 were Applied, no rejections or
Partial outcomes. Standard delivered approximately 240 depth units per arm;
Generous 360, with the declared 120-tick showers and rotating targets. Natural
input totals agree between doses within existing water limits, and 90% natural
input agrees with 0.9 times the matched 100% input. Summed across twelve seeds,
natural input was 72,220.3156 versus 64,998.2840 depth units. Manual input was
0 / 2,880 / 4,320 at either support level. These are physical inputs, not bonus
energy or an activity multiplier.

## Population, births and ancestry

Population means below average the 720 post-step samples per arm and then twelve
seeds equally. They are not tickwise means. Births, deaths and closing counts are
cohort sums. Ancestry is the number of distinct organisms alive at the opening
with living descendants at the endpoint, **not original founder lineages**.

| Natural rain / manual dose | Mean population | Closing population | Births | Starvation / age deaths | Opening ancestry surviving |
| --- | ---: | ---: | ---: | ---: | ---: |
| 100% / none | 96.589 | 1249 | 4679 | 4391 / 144 | 253 |
| 100% / Standard | 96.590 | 1234 | 4653 | 4372 / 152 | 278 |
| 100% / Generous | 96.784 | 1208 | 4643 | 4392 / 148 | 278 |
| 90% / none | 95.741 | 1238 | 4629 | 4337 / 159 | 266 |
| 90% / Standard | 96.592 | 1236 | 4649 | 4369 / 149 | 272 |
| 90% / Generous | 97.494 | 1276 | 4753 | 4432 / 150 | 264 |

No whole-world extinction, collapse death or cap rejection occurred. That is a
two-hour observation, not proof of long-run viability or individual offspring
success. Birth event counts do not reconstruct reproduction payments.

Every seed's matched **90%-minus-100% sampled mean population** follows. Negative
and nearly zero results are retained; the JSON contains each seed's absolute
birth/death, endpoint, form and ancestry results too.

| Seed | No care | Standard | Generous |
| --- | ---: | ---: | ---: |
| 1 | -0.136 | +0.413 | -0.410 |
| 2 | -1.112 | -0.211 | -1.782 |
| 3 | -0.160 | -2.357 | +0.556 |
| 4 | -4.560 | +3.232 | +0.011 |
| 5 | -1.821 | -1.726 | +1.054 |
| 6 | +0.894 | -0.340 | +0.386 |
| 7 | -0.579 | -2.896 | +3.592 |
| 8 | -0.228 | +2.067 | -0.222 |
| 9 | -0.537 | +2.831 | +2.082 |
| 10 | +0.008 | +2.025 | +1.025 |
| 11 | -1.018 | -0.264 | +0.340 |
| 12 | -0.928 | -2.742 | +1.899 |

Without care, 90% lowers mean population in 10/12 seeds: mean difference -0.848,
50 fewer births and 11 fewer closing organisms across the cohort. The corresponding
Standard difference is +0.00255 (5 positive / 7 negative), and Generous +0.711
(9 positive / 3 negative). Within 100%, Standard-minus-none mean population is
+0.00046 (6/12 positive), Generous +0.194 (7/12). Within 90%, they are +0.851
(7/12) and +1.753 (11/12). The mean population differences-of-care-effects are
therefore +0.851 and +1.559, respectively. These are descriptive paired effects,
not a significance test or evidence that repeated rain is an obligation.

## Diversity losses and local water

Forms 0–2 (lantern/grazer, sail/glider, mossback/burrower) stayed nonzero at every
sample in every arm. Form 3 (skimmer) was present initially only in seeds
1, 3, 5 and 10; the other eight absent cases remain in the denominator and never
gain it. Forms 4–7 were absent throughout. Newly sampled skimmer zeros, all still
zero at closing, were:

- 100% none: seed 3; Standard: seeds 1 and 3; Generous: seed 3.
- 90% none: seeds 3, 5 and 10; Standard: seeds 1 and 3; Generous: seeds 3 and 5.

Thus 90% no-care has two additional rare-form losses, even though all total
populations remain alive. Exact first-sampled-zero ticks and every closing form
count are retained; sample timing cannot identify the exact extinction step.

Actual per-tick integrated global water gives mean stocks of
204.822 / 212.864 / 216.885 at 100% and
185.518 / 193.570 / 197.595 at 90% (none / Standard / Generous).
Reducing natural input lowers mean water and flooded-cell time in **all 36**
matched seed/dose pairs. Cohort flooded-cell time falls 15.09% / 14.91% / 14.81%;
time above the drown threshold falls about 26.3% at each dose. These are water
threshold exposures of field cells, not counts of organisms drowned.

At all three target neighborhoods, 90% lowers time-integrated water in every
seed/dose pair. Either manual dose raises it at both support levels in every seed
(all 144 seed/support/dose/target care contrasts). At 100%, the three region mean
water stocks are 9.187 / 9.398 / 12.032 without care, versus
9.663 / 9.947 / 12.791 Standard and 9.914 / 10.233 / 13.151 Generous.
The regions contain 25 / 25 / 16 cells, so raw regional totals are not directly
comparable per-cell habitat quality. Local producer and fruit sampled means and
closing detritus/nutrient stocks are in the artifact; increased water is not
interpreted as ingestion, successful feeding, or universally improved growth.

## Next implication

The proposed axis is physically independent and measurable. Its ecological effect
is small and mixed at this horizon; less flooding did not reliably increase
no-care population and coincided with additional rare-form losses. Preserve the
default and all twelve seeds. If proceeding, extend the **unchanged** paired
recipe to the proposed 24/72-hour horizons, with the same failures, form losses,
no-input controls and fixed audit limits retained. Do not tune the candidate or
combine it with the separate quiet-behavior experiment and call that continuation.
No core, production config, live state, care schedule or canon changed here.

## Unchanged24-hour collection launched

Root read the complete reducer and report, reran all ten tests, and independently
reduced all72 arms with exit0. Its output at
`/tmp/cubarium-root-ambient-two-hour-reduction.json` is byte-identical to the
committed reduction asset, SHA256
`ff9dd52fde1778e316aedac9fbb7e11a9485a46f43aa0ae91b37902dbca14435`.

The same frozen executable (SHA above) is now collecting the preregistered
**24 elapsed simulated hours** after each same mature opening:

```sh
captures/build-cache/ambient-review/release/examples/ambient_compare \
  captures/hunter-openings-2026-09-13 \
  captures/ambient-rain-twenty-four-hour-3e9bc2f \
  --horizon twenty-four-hour
```

Root handle80539 was confirmed live. Log:
`/tmp/cubarium-ambient-twenty-four-hour-3e9bc2f.log`. The actual manifest records
build3e9bc2f, unchanged executable SHA,1,728,000 elapsed ticks,200-tick cadence,
all twelve openings and the same six arms. This is a new longer collection,
not a resumed/relabelled two-hour result. No24-hour outcome or72-hour gate is
claimed yet; the current reducer intentionally refuses the longer contract.
The additional short-screen skimmer losses remain evidence against changing
the live default, which is unchanged.
