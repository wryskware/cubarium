---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Reserve-targets-v1: isolated hunter acquisition experiment

This is an experiment recipe, not selected ecological tuning or permission to
deploy hunters. Wrysk's selected body remains Fable's Lanternjaw; Veilwarden
remains an alternate. The frozen live presentation owner is unchanged.

## Hypothesis and exact intervention

The [stock diagnosis](astra-hunter-stock-bottleneck-proposal-2026-09-13.md)
identifies a mismatch between waiting until reserve falls below 35% before
acquiring prey and requiring 80% reserve to reproduce. Test whether acquiring
successive paid meals at higher reserves improves reproductive stock opportunity.
More hunting may instead spend energy faster or harm prey; births are not promised.

`hunter_compare --profile reserve-targets-v1` changes exactly two profile fields:

| Field | Baseline | Candidate |
| --- | ---: | ---: |
| `seek_reserve_fraction` | 0.35 | 0.80 |
| `perch_reserve_fraction` | 0.65 | 0.90 |

`--profile baseline` remains the default. Both recipes use profile schema/version
3; the manifest recipe and serialized profile SHA distinguish their values.
Neither changes core defaults, phenotype/genome, initial stocks, imported budget,
placement, capture geometry/chance, costs, reproduction gates, 20-second meal
recovery, or facultative scavenging fraction. The two attack-off controls still
disable attacks. Untouched and budget-only controls still have no hunters.

## Paired screen

Use the unchanged twelve aged inputs from
`captures/hunter-openings-2026-09-13/manifest.json`. Run all six arms for each seed,
144,000 elapsed ticks (two simulation hours), care off, no resume, reseeding,
rescue or dropped failures. Run baseline again under this same observer so its
new stock and funding measurements are genuinely measured, not attributed to
the older [completed screen](hunter-profile3-two-hour-results-2026-09-13.md).
Each run requires a new output directory and saves its own executable, build ID,
hashes, profile recipe, opening/closing snapshots and detailed event streams.

Primary comparisons: exact funded/born/refunded/miscarried transactions;
age-and-size-ready member-ticks meeting reserve and energy thresholds separately
and jointly; stock maxima; survival and adult descendants; paid attempts and
cost/capture; inward/outward contact failures; prey integrals, form losses and
paired local recovery. End-of-step stock readiness is not a funding attempt:
quiet behavior, carrying, cooldown, actual funding costs and the population cap
remain separate. Zero-member denominators remain explicit. Do not declare
success from capture totals alone.

The controller's too-near grasp inefficiency remains unchanged. Any later
geometry or energy-budget experiment must have its own named recipe.

## Observer readiness and completion semantics

The exact reproduction observer is branch-hardened through `4fe612b` and its
independent [closure review](astra-reproduction-hardening-review-2026-09-13.md)
(`573d5a5`). Mutation records supply quantities; the observer independently
checks transaction arithmetic, membership and lifecycle reconciliation. It does
not independently measure every heat source inside the core.

`complete_experiment_measurement` now requires at least 144,000 planned ticks,
the exact elapsed horizon, passing numerical audits, no interrupted observer,
and closing-tick coverage of both the arm and reproduction observers. Global
completion additionally requires all six arms of every retained seed. Short
smokes stay false. This certifies the screen's data, never biological acceptance,
self-replacement, long-run stability, or live rollout readiness.

Four additional scalar counters condition reserve/energy readiness on both
reproductive age and adult structure. They retain no per-tick history and do
not mutate the world or consume randomness. Their overlapping member-tick
denominators are distinct from world-tick occupancy.

## Preceding cadence verification

Frozen source `2ac20eb`, built at
`/tmp/cubarium-hunter-audit-frozen-Oq12jd`, passed 66 example tests. Its two
6,000-tick, twelve-seed/six-arm baseline smokes are retained under
`/tmp/cubarium-hunter-audit-cadence-PyXVR7/{w20,w200}`. Root compared the actual
results: all 72 closing state hashes and snapshot SHA values matched across
20/200-tick audit windows; all non-audit summary fields matched, and all twelve
local-recovery and exposure stream pairs were byte-identical. Both cohorts
contained 18 captures and passed technical audits. Their short horizon leaves
measurement completion false. This check predates the two additional branch
guards in `4fe612b` and the recipe above; it is not a full candidate run. The
archive-built executable reports an unknown Git label, so source provenance is
the explicit frozen commit, not an inferred build label.

Recipe implementation tests verify exact two-field isolation for all six arm
profiles, valid profiles, default/explicit/unknown CLI behavior, identical
opening organisms/fields/configuration/import receipts, age-boundary counting,
and completion flags for short, partial, failed and full horizons.

Root reran `cargo test -p cubarium --example hunter_compare` against this recipe:
74 passed, zero failed or ignored. The independent
[recipe review](astra-reserve-target-recipe-review-2026-09-13.md) (`031bcbd`)
likewise passed all 74 and found no blocker to collecting the proposed data.

## Frozen launch checkpoint

Committed recipe: `b547ad086405c8b4fad1361f370001fa16f564f8`. Detached source at
`/tmp/cubarium-reserve-recipe-QkCViu` built the release comparison executable;
its manifest correctly reports `0.1.0+b547ad0`. Baseline and candidate each
completed a 200-tick twelve-seed/six-arm smoke under `/tmp/`:
`cubarium-reserve-recipe-smoke-{baseline,candidate}-b547ad0`. All 144 arms passed
technical/numerical checks; measurement completion stayed false as intended.

All twelve untouched-control summaries were exactly equal across recipes.
Budget-only controls retain the requested hunter profile even with no members,
so their full-state and snapshot hashes intentionally differ; all their other
summary fields, including ecology projection hashes, were exactly equal. Root's
initial overly broad full-summary equality assertion caught this metadata
difference; the corrected check explicitly excludes only those two full-state
hash fields and verifies the budget profile's exact two-field difference.

Full 144,000-tick runs were then started in new directories:

- `captures/hunter-reserve-baseline-two-hour-b547ad0`
- `captures/hunter-reserve-candidate-two-hour-b547ad0`

Each keeps its own frozen executable and manifest. Logs are the corresponding
`/tmp/cubarium-reserve-{baseline,candidate}-two-hour-b547ad0.log` files. These
jobs are in progress at this checkpoint, not completed results. No live process,
opening cohort, prior experiment, or current world state was changed.

Separately, root reran `cargo test --workspace --lib --tests` after Fable's
`a559e54` adapter hardening and the recipe commit: **1,013 passed, zero failed,
16 ignored**, exit 0, across 66 test summaries. Log:
`/tmp/cubarium-lanternjaw-hardening-workspace-tests.log`. This includes the actual
adapter behavior but excludes the example target counted separately above.
The independent [adapter review](astra-hunter-adapter-hardening-review-2026-09-13.md)
closes earlier concrete blockers while retaining endpoint/restart approximations.

## Read-only paired results tool

`node scripts/compare-hunter-recipes.mjs BASELINE_DIR CANDIDATE_DIR` reads only
completed twelve-seed/six-arm outputs. It checks manifest recipe identity,
matching executable/cohort/configuration, completion and observer denominators,
numerical peak summaries, saved snapshot CRC/SHA/full-state hashes, exact
two-field profile isolation, event/offspring/maturity counts, and selected local
recovery counts. Untouched controls must match completely; budget-only controls
may differ only in the two full-state hashes that include their stored profile.
The report keeps every seed and arm, uses null for no mature-member denominator,
and separates stock readiness from actual funding, births, survival and prey loss.

This is artifact reduction and consistency checking, not a rerun of numerical
audits or independent reconstruction of contact geometry and transaction amounts.
Snapshot checks validate the envelope and payload hash, not semantic WorldState.
The shared snapshot helper still requires schema9 by default for preparation;
the report explicitly selects schema11 because these experiments were frozen
before the in-progress care-dose schema change.

Seven new reduction tests and six existing preparation tests pass. A direct
read of the first eight completed seeds in each run checked 96 actual arm
artifacts using the helper checks, including CRC/SHA/full-state hashes, events,
coverage and paired opening recipes. The full-report command correctly refused
the still-incomplete cohort (global summary absent); no completed aggregate or
candidate acceptance is inferred from those partial checks.
