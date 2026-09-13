---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Corrected twelve-hour repeats preserve the old ecology

Both frozen schema9 comparisons completed with exit0: seeds1 and3, each with
untreated and repeated-care arms,864000ticks/12hours, care cycle2400ticks and
independent observer window200ticks. Core `b47eacc`, harness `57408c4`, executable
SHA256 `9f317241c96865c13a8532eab2c76fc47fc9737843b484e76f212e97ae0058e6`.
Artifacts are in ignored `captures/care-longitudinal-2026-09-13/`:
`seed1-repeated-compensated-window200.json` and
`seed3-repeated-compensated-window200.json`. Completed root handles41258/74507.

| Seed / arm | Raw legacy peak | Corrected peak | Independent windowed peak | Raw gate / new joint gate |
| --- | --- | --- | --- | --- |
| 1 untreated | 1.6832076653e-5 | 2.3328539100e-10 | 6.4028427005e-10 | pass / pass |
| 1 cared | 2.0506936266e-5 | 3.5709035728e-10 | 6.2811977841e-10 | fail / pass |
| 3 untreated | 1.9809653168e-5 | 2.0838797354e-10 | 5.8935256675e-10 | fail / pass |
| 3 cared | 2.1090722953e-5 | 1.4460965758e-10 | 4.6003378884e-10 | fail / pass |

Energy limits are unchanged:1.9563930834586353e-5 for seed1 and
1.973537879254678e-5 for seed3. Maximum immediate care energy error is
4.547473508864641e-13. Material peaks stay below9.66e-10; water peaks below
2.15e-9 under the same original1e-8 water limit. The new gate requires persisted
compensated energy AND independent energy AND material/water AND care-boundary
checks, as documented in `corrected-care-audit-2026-09-13.md`. No old failed
diagnostic was relabeled passing and no tolerance was relaxed.

## Preservation checks against the actual old reports

Root compared each arm against `seed1-repeated-window200.json` and
`seed3-repeated-window200.json` from the frozen pre-correction core. Deep equality
holds for every receipt, care ledger, raw cumulative energy flow, raw residual
maximum, population minimum/maximum, census sample, extinction record, surviving
opening cohort count and maximum descendant depth. Complete final telemetry is
also equal except the intentionally changed schema9 full-state hash. In particular,
the four ecology hashes still match exactly:

| Seed / arm | Old and new ecology hash |
| --- | --- |
| 1 untreated | 559601584664565610 |
| 1 cared | 394482520113116252 |
| 3 untreated | 3654769590440006151 |
| 3 cared | 17062153529389062666 |

The final populations and form losses therefore remain exactly those of the prior
twelve-hour evidence. Accurate accounting does not repair missing skimmers,
validate predator prey-recovery margins, or establish ecological stability beyond
the measured horizon. Genuine pre-change schema7/8 continuation tests and restart
tests are separate evidence reviewed in `1d7b386`.

## Remaining schedule checks

Started with the SAME frozen corrected executable:

- Seed1 occasional, care cycle12000ticks: root exec98612,
  `seed1-occasional-compensated-window200.json`.
- Seed2 occasional, care cycle12000ticks: root exec77787,
  `seed2-occasional-compensated-window200.json`.

Each is another matched12h comparison. Seed1 has a complete old report for exact
comparison; seed2's older occasional attempt failed before emitting JSON, so no
missing old ecology can be invented. These handles were live at launch; poll them
before reporting completion. No live state, user care or display transport was
changed for any of these experiments.
