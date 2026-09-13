---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Paid charging: real births, but a technically failed paired study

**Do not accept this as a completed twelve-seed experiment.** The background
finished all twelve seeds (authoritative root process 58224, exit 0); the charging
candidate finished its driver with exit 1 (43911), retaining a seed-6 core
invariant failure. The unchanged strict `compare-hunter-recipes.mjs … charge80`
command also returned exit 1. It was not weakened to produce a passing reduction.

The useful observation is narrower: paid charging opened mature energy gates and
produced real, fully paid offspring. It did **not** demonstrate self-replacement:
no observed descendant reached adult structure, and most founders still died.
No ecology parameter, live world, renderer, or canonical decision changed here.
Lore/Graft/canon and the [charging proposal](astra-hunter-paid-charging-proposal-2026-09-13.md)
were consulted.

## Provenance and trustworthy boundaries

Original directories remain immutable:

- `captures/hunter-charge-background-two-hour-3b06596`
- `captures/hunter-charge-candidate-two-hour-3b06596`

Both used build `0.1.0+3b06596`, frozen executable SHA256
`351997076c8a3e7a032109c45eee74541c75a87339a967018b083b23dd8674e0`.
Same twelve schema-9 mature openings, six arms, 144000 planned ticks, no care;
candidate changes only semantic profile version 3→4, selecting paid oxidation
below 0.80 rather than 0.50 on the fixed 0.80/0.90 reserve-target background.

The [read-only extraction](astra-charge-result-reduce-2026-09-13.mjs) retains the
strict comparison refusal and **138 complete arms plus six incomplete candidate
seed-6 arms**, not a fabricated 144-arm pass. It checks executable identity,
both snapshot envelopes/CRC/SHA/state hashes per arm, matched manifest/opening
configuration and imports, exact version-only profile pairing, recorded event
counts, oxidation bounds and completed-arm audit/observer coverage. Complete
arms' maximum peak/fixed-limit ratio is 0.010164 (water). These are retained audit
checks, not independent reconstruction of every transfer. A failed state's valid
CRC does not make its semantic state valid.

All six candidate seed-6 worlds stop at tick 170772 (elapsed 26772, 22.31 minutes).
Local paired observations are trusted only through 170771. The offending
facultative arm's normal observer also stops at 170771; its final state already
contains a further capture. Thus recorded prefix capture count 18 is **not** a
claim that the final world has only 18 captures. No two-hour mean or survival
comparison is calculated from this shortened prefix.

The [saved extraction](assets/astra-charge-result-reduction-2026-09-13.json)
keeps every seed/arm, raw readiness denominators, null opportunities, costs,
form losses, local statuses, child identities and explicit incomplete markers.
Descriptive pooled comparisons below use the **eleven complete matched pairs**,
always excluding seed 6 from both sides. This is a labeled observation subset,
not a replacement population or an ecological success gate.

## Exact failure mechanism, independently replayed

The isolated [replay example](../../crates/cubarium/examples/astra_charge_failure.rs)
resumes the original seed-6 facultative post-initialization snapshot and advances
26772 ticks with unchanged biology. It reproduced the **entire closing snapshot
byte-for-byte**, state hash `9140998897574537509`; ordinary decoding correctly
rejects it with `hunter 29:6: stalking no one`. The
[two-boundary replay output](assets/astra-charge-failure-replay-2026-09-13.jsonl)
contains the exact before/after members and events absent from the failed
harness observer's last boundary.

- Tick 170739: parent `29:6` produces paid child `74:5` (funding key started
  168338; this is a completed gestation, not an unpaid founder).
- Tick 170771: parent is **Stalking** prey `63:5`; child is already in a paid
  **Strike** against that same prey, ending 170772.
- Tick 170772: child captures prey for `0.6035863309232207` material and
  `1.0122994782514116` chemical energy, following the paid `0.08` strike.
  Capture cleanup in `world.rs` clears the other hunter's target but does not
  change its unpaid Stalking phase. Parent becomes **Stalking + None**, rejected
  by `HunterState::validate`. The child enters Handling and digestion proceeds.

This is a multi-hunter target-invalidation defect exposed by successful breeding,
not evidence that charging manufactured resources or that births should be
disabled. Debug builds panic at the same completed boundary; the diagnostic
catches only that expected final tick to preserve the state, then requires exact
old bytes and decoder rejection. It exited 0 on the pre-fix source. It is an
old-failure reproducer, not a regression test expected to keep passing after repair.

## Every seed's hunting and founder survival

Each cell: **captures background→candidate; candidate births; founder lifetime
minutes background→candidate**. All background births are zero. `≥120` is alive
at the planned horizon, not a measured death time. Seed 6 is explicitly a shorter
failed prefix; its two live hunting founders are censored, not survivors to 120.

| Seed | Specialist on | Facultative on |
| --- | --- | --- |
| 1 | 28→114; 3; 38.40→≥120 | 68→114; 3; 86.97→≥120 |
| 2 | 16→10; 0; 22.06→18.50 | 21→119; 3; 26.07→≥120 |
| 3 | 15→26; 0; 25.14→29.04 | 90→24; 0; 91.14→28.78 |
| 4 | 26→12; 0; 33.89→16.07 | 15→8; 0; 17.57→13.32 |
| 5 | 58→33; 1; 70.02→35.92 | 85→44; 1; 94.50→54.47 |
| 6, failed | 25→17 recorded; 0; 31.00→censored 22.31 | 25→18 recorded; 1; 31.00→censored 22.31 |
| 7 | 65→32; 1; 77.57→36.32 | 61→58; 1; 79.14→58.35 |
| 8 | 26→47; 1; 29.01→33.04 | 25→28; 1; 28.60→28.56 |
| 9 | 3→4; 0; 8.66→9.44 | 3→6; 0; 9.30→13.01 |
| 10 | 24→57; 0; 27.23→60.35 | 75→19; 0; 83.33→27.54 |
| 11 | 11→12; 0; 18.92→22.17 | 11→13; 0; 18.92→22.30 |
| 12 | 34→53; 0; 41.85→51.68 | 30→82; 2; 42.16→76.14 |

The previous reserve-target candidate and this background reproduce the same
listed capture/lifetime results. The charging effect is mixed, not a universal
survival improvement: for example seed-3 facultative loses over an hour despite
other seeds' surviving founders.

## Mature stock opportunities and paid offspring

Candidate percentages below are **reserve / energy / both stock gates open**,
conditioned on age **and** adult structure, observed each end-of-step. `—` means
zero mature denominator. End-of-step stock opportunities are not funding records:
the funding transaction itself debits stocks before observation.

| Seed | Specialist candidate % | Facultative candidate % |
| --- | --- | --- |
| 1 | 21.03 / 75.92 / 21.03 | 21.85 / 75.99 / 21.85 |
| 2 | — / — / — | 17.69 / 76.76 / 17.69 |
| 3 | 0 / 15.19 / 0 | 0 / 23.87 / 0 |
| 4 | — / — / — | — / — / — |
| 5 | 0.005 / 59.26 / 0.005 | 8.29 / 71.95 / 8.29 |
| 6 | Incomplete | Incomplete |
| 7 | 0.12 / 50.79 / 0.12 | 22.01 / 67.46 / 22.01 |
| 8 | 1.34 / 46.58 / 1.34 | 0.42 / 8.47 / 0.42 |
| 9 | — / — / — | — / — / — |
| 10 | 0 / 54.13 / 0 | 0 / 0 / 0 |
| 11 | 0 / 0 / 0 | 0 / 0 / 0 |
| 12 | 0 / 60.68 / 0 | 17.01 / 74.49 / 17.01 |

For the eleven complete pairs, mature member-ticks S/F are
222194/446311 background and 274225/427369 candidate. Mature reserve-ready counts
change 52200→25468 S and 66402→72508 F. Mature energy-ready counts change
0→170585 S and 0→297686 F; candidate joint counts are 25468/72508. This supports
the intended battery mechanism, while showing the coupled reserve constraint
persists. Some energy-ready worlds never have reserve readiness.

Complete candidate arms contain **17 funded and 17 born** transactions (6 S,
11 F); seed-6 prefix adds one funded/born. Every one of the 18 retained Funded
records pays exactly 1.6 reserve and 1.0 battery (floating debit checks within
1e-12), stores 0.8 structure/0.8 reserve/0.6 energy, and books 0.4 construction
heat; every Born books 1.6 birth heat. There are no refunds, miscarriages, open
gestations or reproducing descendants in these records.

Of the 17 complete-arm children, **16 died of Starvation**, with lifetimes
226.3–1131.5 seconds. One seed-2 facultative child is right-censored alive at
144.65 seconds; seed-6's 1.65-second child is separately technically censored.
No child reached adult structure. Every sampled child structure stays at its
birth value 0.8 versus adult 2.0; this is sampled evidence, not a tickwise growth
ledger. The seed-8 specialist child makes 13 recorded captures yet still dies
without observed growth. Sampled child reserve maxima never exceed 1.02943,
below the inherited ordinary growth activation at 0.3×4=1.2. This points to a
juvenile acquisition/allocation bottleneck, not simply “they never catch food.”

Across complete arms adult occupancy is exclusively zero or one, never two or
more; three founders remain alive (seed-1 S/F and seed-2 F). Closing total hunters
are four because one surviving child remains juvenile. Rarity is therefore not
proof of a self-maintaining rare adult population.

## Costs, controls and prey: no free improvement

Charging diagnostics count actual candidate transactions above the reference
activation threshold on the **realized candidate path**. They are not the net
extra oxidation relative to a counterfactual trajectory. Eleven-pair totals:

| Candidate arm | Reserve burned | Battery gained | Conversion heat |
| --- | ---: | ---: | ---: |
| Specialist off | 22.000 | 35.200 | 8.800 |
| Specialist on | 143.421 | 229.473 | 57.368 |
| Facultative off | 22.197 | 35.516 | 8.879 |
| Facultative on | 189.128 | 302.604 | 75.651 |

Each burn returns material to local nutrient and pays conversion loss. Off
hunters still all starve before reproductive age; their living trajectories may
change under charging and are **not** identity controls. Mean off lifetime is
13.689→13.689 minutes S and 14.062→13.897 F. Untouched summaries match exactly
and non-member budget controls match after only the documented policy/hash
projection in each of the eleven complete pairs. Seed-6 closing states cannot be
compared to background at a different elapsed time.

Enabled captures total 306→400 S and 484→515 F; paid strike energy
115.76→136.32 S and 173.84→174.32 F. Most OutOfReach settlements remain inward
(candidate 750/832 S, 938/1049 F), but charging did not modify geometry, and
settlement classification does not establish admission causality.

Mean prey over the matched 11×144000 world-ticks changes 95.178→94.930 S and
94.915→94.567 F; closing prey sums change 1131→1127 S and 1174→1134 F.
Effects vary by seed and form. Form-1 below-half exposure falls 233676→54366
ticks S (6→5 seeds), but rises 243427→297890 F (7→7 seeds). Form-3 newly-zero
seeds change [3,5]→[1,3] S and [3]→[1] F; untouched already loses form 3 in
seed 3, so these are not automatically hunter-caused extinctions. All form and
local-recovery statuses—including no opportunity and censoring—remain in the
extraction; none establish harmless predation or a complete paired recovery pass.

## Next action: one causal correction, no parameter retuning

First repair **shared-prey target invalidation with phase consistency**. Cover
the exact parent-Stalking/child-capture case and the related target cleanup
helpers; unpaid Stalking/Windup may end coherently, but a paid Strike must retain
its normal failed-settlement accounting and episode identity. Do not fix this by
weakening validation, erasing the failed artifacts, deleting newborns or granting
resources. Root delegated that narrow core/test work separately.

After focused regression/restart/no-hunter identity checks, repeat the **same**
fixed charging/background recipe on all twelve inputs in a new frozen run.
Preserve this failed study as evidence. Only after a technically complete repeat
should the sampled juvenile acquisition-versus-growth bottleneck justify a new,
separate causal measurement or experiment. No threshold, meal, geometry or
offspring-cost changes are bundled into this recommendation.

Verification: original strict CLI exit 1 retained; failure-aware extractor exit 0
means its labeled evidence checks completed, **not** experimental acceptance;
eleven original reducer unit tests passed; exact failure replay exited 0 and
matched old snapshot bytes. This report does not claim a repaired-core run,
24/72-hour stability or a live introduction gate.
