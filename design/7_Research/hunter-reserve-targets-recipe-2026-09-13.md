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
74 passed, zero failed or ignored. Independent recipe review and the full paired
runs are still pending; no candidate outcome is reported here.
