---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Complete twelve-seed reserve-target comparison

**Outcome: the reserve-acquisition hypothesis partly worked; the lineage still
failed self-replacement.** Raising seek/perch reserve targets from 0.35/0.65 to
0.80/0.90 increased captures, average survival and mature reserve readiness, but
both recipes produced **zero funded gestations, zero offspring and zero surviving
hunters at two hours**. Every introduced hunter died of Starvation. This is not a
viable reproducing population or a live-introduction recommendation.

Fable's Lanternjaw remains the selected visual body. Geometry, energy economy,
ambient support and biological viability are separate from that visual choice.

## Provenance and completeness

Read and reduced both immutable result directories:

- `captures/hunter-reserve-baseline-two-hour-b547ad0`
- `captures/hunter-reserve-candidate-two-hour-b547ad0`

Both are build `0.1.0+b547ad0`, frozen executable SHA256
`92346cb413d0b603cefe73dd29880645cb214c1ca18a90a6bd2c488b48ec73f1`,
observer contract `exact-reproduction-v2`. All twelve aged inputs and all six
arms were retained, 144,000 elapsed ticks at 20 Hz from absolute tick 144000 to
288000. Care was off. No rescue, added founders, reseeding or discarded failure
is part of these runs. Opening prey ranged 78–101, stratified low/middle/high.

The reviewed `scripts/compare-hunter-recipes.mjs` completed successfully across
**144 arm artifacts**. It verified both completion flags, observer/reproduction
closing coverage, fixed-limit summaries, executable hashes, snapshot envelope
CRC/SHA/payload-state hashes, event/count reconciliation, matched cohort and
imports, and exact two-field recipe isolation. All final local paired observers
also reach tick 288000. Both logs end `seed12/12 complete: planned horizon`.

Untouched-control summaries match exactly. Budget-only summaries match after
excluding the two hashes that encode the stored differing profile; the reducer
explicitly verifies that exception. I additionally checked the 24 attack-off
summary pairs: all fields also match except those same profile-bearing hashes.
Thus the changed outcomes are confined to enabled hunting in this actual screen.

Across recorded peak audits, the largest peak/limit ratio is water at 0.010164
(about 1.02% of its fixed limit); material and either energy audit are much smaller.
All are finite and strictly below limits. These are checks of completed audit
evidence, not a fresh simulation run or independent reconstruction of every core
transfer. Numerical completion does not imply biological acceptance.

Saved full, all-seed/all-arm reduction:
[`astra-hunter-reserve-paired-reduction-2026-09-13.json`](assets/astra-hunter-reserve-paired-reduction-2026-09-13.json).
It retains per-seed null denominators, exact counts, costs, prey/form metrics and
local status counts. Originals were not changed. Reproduce with:

```text
node scripts/compare-hunter-recipes.mjs captures/hunter-reserve-baseline-two-hour-b547ad0 captures/hunter-reserve-candidate-two-hour-b547ad0
```

The reducer's nine focused tests also passed. Additional analyses below read the
original summaries, census and event files; no world, core, art, runner or live
process was changed. Lore and current canon were consulted; this is evidence,
not a new accepted design decision.

## Six-arm biological outcomes

S/F mean specialist/facultative; off/on mean attacks disabled/enabled. B→C means
baseline→candidate. Lifetime is elapsed time after introduction, not absolute
world age. All introduced founders were adult-size; none lived to the horizon,
so their lifetimes are observed deaths rather than right-censored survivals.

| Arm | Captures B→C | Mean hunter lifetime, minutes B→C | Mean prey over time B→C | Closing prey sum B→C |
| --- | ---: | ---: | ---: | ---: |
| Untouched | 0→0 | Not applicable | 96.580→96.580 | 1249→1249 |
| Budget control | 0→0 | Not applicable | 96.233→96.233 | 1266→1266 |
| S off | 0→0 | 13.690→13.690 | 96.307→96.307 | 1242→1242 |
| S on | 179→331 | 25.392→35.312 | 96.637→94.590 | 1259→1218 |
| F off | 0→0 | 14.116→14.116 | 96.098→96.098 | 1241→1241 |
| F on | 300→509 | 37.861→50.725 | 95.792→94.349 | 1240→1261 |

Each mean prey denominator is 12 × 144000 world-ticks. It counts non-hunters by
membership. Off controls have no mature-age member-ticks: they die before the
20-minute reproductive age, not because their founder structure is juvenile.

Across both recipes: **96 introduced founders, 96 Starvation deaths**. In every
arm, Funded/Born/Refunded/Miscarried, NotFunded, open gestations, adult descendants
and descendants reproducing are all zero. There are no hidden funded-and-lost
gestations behind the absent births. Zero NotFunded records is not successful
funding: the local reproductive gate never opened, so it never reached those
mutation-site funding refusals.

Occupied hunter time is entirely one imported adult, then zero; no arm ever has
two or more adults. Sparse occupancy alone is therefore an extinction artifact,
not successful sparse self-replacement.

### Every seed's hunting outcome

Each cell is **captures / lifetime in minutes**, B→C. No seed is omitted.

| Seed | S on B→C | F on B→C |
| --- | --- | --- |
| 1 | 16 / 31.79 → 28 / 38.40 | 34 / 48.89 → 68 / 86.97 |
| 2 | 1 / 10.14 → 16 / 22.06 | 16 / 24.54 → 21 / 26.08 |
| 3 | 12 / 19.81 → 15 / 25.14 | 34 / 55.56 → 90 / 91.14 |
| 4 | 28 / 32.21 → 26 / 33.89 | 31 / 39.12 → 15 / 17.57 |
| 5 | 18 / 33.98 → 58 / 70.02 | 20 / 35.23 → 85 / 94.50 |
| 6 | 11 / 23.26 → 25 / 31.00 | 96 / 115.10 → 25 / 31.00 |
| 7 | 15 / 24.88 → 65 / 77.57 | 9 / 19.86 → 61 / 79.15 |
| 8 | 15 / 25.14 → 26 / 29.01 | 20 / 31.55 → 25 / 28.60 |
| 9 | 0 / 8.89 → 3 / 8.66 | 1 / 10.14 → 3 / 9.30 |
| 10 | 48 / 57.89 → 24 / 27.23 | 14 / 26.40 → 75 / 83.33 |
| 11 | 8 / 18.09 → 11 / 18.92 | 11 / 23.72 → 11 / 18.92 |
| 12 | 7 / 18.63 → 34 / 41.85 | 14 / 24.23 → 30 / 42.16 |

Candidate survival improves in 10/12 specialist and 7/12 facultative pairs, but
not universally: seed 6 facultative loses about 84 minutes. Its former high
capture count is not a reason to exclude that counterexample. More captures also
need not prolong life (seed 9 both variants; seed 8 facultative).

## Readiness: material improves; the mature energy gate never opens

These are overlapping **end-of-step member-tick** diagnostics conditional on
reproductive age and adult structure, not attempts, births or independent samples.

| Recipe/arm | Mature member-ticks | Reserve gate open | Energy gate open | Both open |
| --- | ---: | ---: | ---: | ---: |
| B S on | 106959 | 0 | 0 | 0 |
| C S on | 235391 | 52200 (22.18%) | 0 | 0 |
| B F on | 269190 | 0 | 0 | 0 |
| C F on | 459508 | 66402 (14.45%) | 0 | 0 |

Candidate mature reserve opportunity by seed, percent; **— means no mature
denominator**, not zero-percent opportunity. Mature energy/joint readiness is
zero whenever a mature denominator exists in either recipe.

| Seed | C S reserve-ready % | C F reserve-ready % |
| --- | ---: | ---: |
| 1 | 21.11 | 23.17 |
| 2 | 0 | 0 |
| 3 | 0 | 0 |
| 4 | 19.64 | — |
| 5 | 25.79 | 13.36 |
| 6 | 0 | 0 |
| 7 | 31.33 | 13.73 |
| 8 | 8.14 | 8.52 |
| 9 | — | — |
| 10 | 0 | 24.95 |
| 11 | — | — |
| 12 | 23.89 | 23.54 |

Candidate global reserve maxima reach 0.9556 S / 0.9703 F, versus baseline
0.5090 / 0.5484. Candidate global energy maximum 0.7884 exceeds the 0.75 gate,
but occurs **before reproductive age**; it is not evidence of mature readiness.
The every-tick conditioned energy count, not that lifetime maximum, establishes
the mature obstruction. As corroborating lower-resolution evidence, mature
census maxima are 0.56345 S / 0.57527 F, including observations with reserve gate
open. These census maxima are sampled, not exhaustive per-tick maxima.

The mechanism remains consistent with the earlier stock diagnosis: oxidation
only activates below 0.5 battery fraction, while reproduction requires 0.75.
Having reserve above 0.8 does not automatically charge a reproductive battery.
This is source-based interpretation plus observed readiness, not proof that
changing one threshold would yield a sustainable lineage.

## Paid effort and the unchanged geometry inefficiency

| Recipe/arm | Paid settled attempts | Captures | OutOfReach (inward / outward) | Paid strike energy | Energy/capture |
| --- | ---: | ---: | ---: | ---: | ---: |
| B S on | 854 | 179 | 463 (430 / 33) | 68.32 | 0.38168 |
| C S on | 1569 | 331 | 848 (776 / 72) | 125.52 | 0.37921 |
| B F on | 1313 | 300 | 673 (633 / 40) | 105.04 | 0.35013 |
| C F on | 2295 | 509 | 1162 (1095 / 67) | 183.60 | 0.36071 |

Capture share of paid attempts barely changes: 20.96→21.10% S and
22.85→22.18% F. Candidate produces 385/621 missed rolls after valid contact,
29/28 unpaid Unaffordable refusals, and 5/3 unmapped grasps. Baseline also has
one lost-target settlement in S. Captures per pooled living-hunter hour rise
35.25→46.87 S and 39.62→50.17 F: it hunts more intensively, not more efficiently.

Candidate captured inventories total 320.237m / 400.135e S and
494.543m / 609.299e F. Applying the unchanged digestion arithmetic to each
actual captured M/Q gives optimistic direct battery potentials of 82.193e S and
125.388e F, below respective strike payments 125.52e / 183.60e even before
maintenance, sensing, movement and handling. Additional reserve can later be
oxidized; facultative field intake is not included. These are food-potential
calculations, **not measured net profit or an isolated hunter heat ledger**.

Inward classifications describe settlement body coordinates relative to the claw,
not the cause of admission. Nevertheless 91.5% S / 94.2% F of candidate OutOfReach
settlements remain inward. Raising reserve targets did not repair that separate
controller/geometry inefficiency.

## Prey totals do not erase form-specific damage

Every seed and all six arms' **closing non-hunter prey**, B/C (equal controls shown
once). Total prey never goes below half its own opening count; the observer's
`no_decline` status means no such threshold crossing, not a flat population.

| Seed | Untouched | Budget | S off | S on B/C | F off | F on B/C |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 108 | 97 | 93 | 93/95 | 96 | 89/93 |
| 2 | 108 | 115 | 113 | 102/113 | 113 | 114/110 |
| 3 | 99 | 97 | 102 | 95/92 | 97 | 94/103 |
| 4 | 120 | 113 | 114 | 107/110 | 127 | 109/108 |
| 5 | 100 | 92 | 102 | 102/92 | 102 | 84/103 |
| 6 | 89 | 98 | 92 | 86/87 | 87 | 80/87 |
| 7 | 116 | 126 | 116 | 113/109 | 117 | 122/124 |
| 8 | 103 | 113 | 100 | 100/101 | 104 | 110/113 |
| 9 | 97 | 106 | 104 | 96/97 | 94 | 109/101 |
| 10 | 118 | 114 | 118 | 140/121 | 127 | 122/119 |
| 11 | 92 | 90 | 90 | 97/101 | 90 | 99/101 |
| 12 | 99 | 105 | 98 | 128/100 | 87 | 108/99 |

Candidate mean prey over time falls relative to baseline in 10/12 specialist and
8/12 facultative pairs. Pooled losses are 2.046 and 1.444 mean prey respectively.
The facultative closing sum nevertheless rises by 21: a final count alone misses
the lower accumulated prey occupancy and changed trajectories.

Form 1 falls below half its opening count in 6/12 B S and 6/12 C S, versus 0/12
S off; in 4/12 B F and **7/12 C F**, versus 1/12 F off. Duration below half grows
from 149559→233676 form-ticks S and 139296→243427 F (0 and 14024 respectively in
the unchanged off controls). It is not enough to cite only extinction counts.
No form 0–2 reaches zero. Form 3 is present in only seeds 1, 3, 5, 10 at opening;
its new zero events are:

| Arm | B seeds newly zero | C seeds newly zero |
| --- | --- | --- |
| Untouched | 3 | 3 |
| Budget | 3, 5 | 3, 5 |
| S off | 1, 3, 5 | 1, 3, 5 |
| S on | 3, 10 | 3, 5 |
| F off | 1, 3, 5, 10 | 1, 3, 5, 10 |
| F on | 3, 10 | 3 |

Forms 4–7 were absent at opening; do not count them as hunter-caused losses.
Form-zero events are not the same as extinction of every original prey lineage.
The different signs across these controls do not establish harmless predation:
competition, reproduction, movement and ordinary deaths all continue.

## Paired local recovery, with every outcome retained

Graph-radius-3 neighborhoods and first-capture-per-600-second-bin selection are
unchanged. Each selected site's same cells are followed in all six arms.

| Recipe/arm | Captures / selected | Recovered | No measured deficit | Insufficient pre-history | Not recovered in 1 h | Right-censored |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| B S on | 179 / 32 | 30 | 1 | 0 | 1 | 0 |
| C S on | 331 / 43 | 23 | 7 | 11 | 2 | 0 |
| B F on | 300 / 48 | 41 | 3 | 0 | 2 | 2 |
| C F on | 509 / 63 | 44 | 5 | 11 | 2 | 1 |

The candidate's earlier acquisition creates 11 insufficient-prehistory windows
per variant; those are **not failed recovery or successful recovery**. Unselected
captures (147/288 S, 252/446 F, B/C) remain recorded as exposures, not erased.
Candidate recurrent-exposure recoveries are 17 S and 39 F; single-exposure
recoveries are 6 and 5. Both one-hour non-recoveries in each candidate variant
have recurrent exposure. Persistent predation can therefore overlap the recovery
window; this is not isolated single-kill causality.

These are treatment-selected occupancy sites, not random samples, and the selected
sites differ across recipes. Do not compare 23/43 versus 30/32 as an unbiased
recovery-rate effect, divide recoveries by all captures, or treat no-deficit and
censored windows as successes. Recovery of occupancy is not proof that the eaten
lineage or form was replaced; per-form local recovery is not measured here.

## Proposed next experiment: one forward-sweep admission family

**Do not deploy this candidate or extend it as a claimed viable lineage.** Keep
it as the completed acquisition-policy result. For the next paired screen, propose
`forward-sweep-entry-v1` against the unchanged `reserve-targets-v1` reference:
change **only the inward boundary of admission from Stalking into Windup**.

Concrete candidate predicate, in the existing root-owned contact chart:

```text
existing: effector_distance <= tolerance + closing
candidate: existing AND body.x >= capture_offset_body.x + closing - tolerance
closing = the existing strike_closing_px(profile)
```

This is a conservative signed full-forward-sweep hypothesis: for a stationary
prey and the planned forward burst, starting inward of that boundary cannot end
within the unchanged grasp tolerance. The old symmetric admission disk adds
burst reach on the inward side even though the burst moves the claw forward.
Actual prey movement, wading, heading changes and the existing stopping rule
mean this is **not a complete reachable-set controller or guaranteed fix**.

Keep seek/perch 0.80/0.90, physical claw and reach, root steering, nearest-target
selection, stopping/retreat behavior, strike speed/duration/roll/cost, sensing,
digestion/recovery, reproductive gates/payment, body/genome, imports, care-off
schedule and all twelve inputs unchanged. Do not add retreat, enlarge tolerance,
lower reproduction thresholds or cut energetic costs in the same experiment.
Use a named profile/recipe flag, not a global silent behavior change.

Add bounded observational counts/evidence at admission and payment, including
rejected inward admission and the actual paid-entry measure, without consuming
RNG. Settlement near/far evidence alone cannot say how many failures this rule
would prevent. Preserve all six arms and rerun both reference and candidate under
the same observer and executable. Positive controls must show unchanged default
behavior, exact charges and genuine eligible captures; topology checks must use
the current root-owned geometry, not independently transported mouth anchors.

Primary prediction: fewer inward failed paid strikes and lower paid energy per
capture, without merely preventing hunting and shortening life. Measure mature
R/E/joint opportunities, exact funding/birth/miscarriage, descendants, survival,
prey/form burden and local recovery exactly as here. The energy gate may remain
closed; improved strike efficiency alone is not the acceptance criterion. Any
later energy-policy experiment is another named family, not a hidden addition
to this one. This proposal authorizes no implementation or live action by itself.
