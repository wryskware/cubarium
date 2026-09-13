---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Profile 3: all twelve seeds, all six arms, two hours

**Completed numerical screen, unsuccessful self-replacing lineage.** All 72 arms
reached their planned horizon and passed the fixed material, energy and water
audit limits. The two hunting variants made 479 captures in total, but every
introduced hunter died of starvation and no hunter offspring appeared. This
profile is not ready for the live cube. No seed was removed, reseeded or rescued.

## Provenance and verification

Frozen source `9eacb7e`, build `0.1.0+9eacb7e`, executable SHA256
`4906811c15fbf680fe61840d109eae0a7e040ddaaa1121f11661c79432506aa3`.
The process completed with terminal exit **0**. Full evidence is under
`captures/hunter-profile3-two-hour-2026-09-13/`: immutable manifest, frozen
executable, per-arm opening/closing snapshots, event streams, sparse census,
seed summaries, all-capture exposures and selected local recovery records.

Every seed began from its prescribed schema-9 aged opening at tick 144000;
the fixed founder/budget import began the elapsed trial. Every arm closed at
tick 288000 after **144000 elapsed ticks = two hours**. Care was disabled.
The six arms are untouched, matched resource deposit, specialist attack-off/on,
and facultative attack-off/on. The founder was the prescribed one per treatment
arm; there was no two-adult cap. See the [experiment contract](hunter-experiment-contract-2026-09-13.md).

Root checked the completed manifest/summary, all 72 per-arm completion/audit flags,
all closing ticks and observer coverage, SHA256 of **all 72 closing snapshots**
against their recorded checksums, and the frozen executable checksum. Every
adult-occupancy denominator sums to 144000. Per-seed capture counts match the
paired observer's exposure counts; streamed selected-window counts match its
selected denominators. This verifies artifact consistency; it does not substitute
for an independent re-execution of every arm.

Largest absolute residuals across the complete set:

| Quantity | Peak residual |
| --- | ---: |
| Material | 1.7485035641584545e-10 |
| Independent energy | 4.722657820366294e-10 |
| Corrected stored/ledger energy | 7.833023119019344e-11 |
| Water | 5.711626727133989e-10 |

The frozen executable predates exact reproduction mutation records. Its
`complete_experiment_measurement` remains **false**, even though its declared
numerical screen completed. No observed escrow starts or births occurred; this
old observer cannot rule out an escrow funded and lost within one step. The
[new integration](hunter-reproduction-observer-integration-2026-09-13.md) must be
reviewed and exercised separately, not retroactively credited to this run.

## All-seed closing prey and captures

Counts below are **non-hunters**, by explicit membership. All hunter closing
counts, offspring counts and adult-descendant counts are zero in all arms.
S/F denote specialist/facultative; Off/On denote attacks disabled/enabled.

| Seed | Untouched | Deposit | S Off | S On | F Off | F On | S captures | F captures |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 108 | 97 | 93 | 93 | 96 | 89 | 16 | 34 |
| 2 | 108 | 115 | 113 | 102 | 113 | 114 | 1 | 16 |
| 3 | 99 | 97 | 102 | 95 | 97 | 94 | 12 | 34 |
| 4 | 120 | 113 | 114 | 107 | 127 | 109 | 28 | 31 |
| 5 | 100 | 92 | 102 | 102 | 102 | 84 | 18 | 20 |
| 6 | 89 | 98 | 92 | 86 | 87 | 80 | 11 | 96 |
| 7 | 116 | 126 | 116 | 113 | 117 | 122 | 15 | 9 |
| 8 | 103 | 113 | 100 | 100 | 104 | 110 | 15 | 20 |
| 9 | 97 | 106 | 104 | 96 | 94 | 109 | 0 | 1 |
| 10 | 118 | 114 | 118 | 140 | 127 | 122 | 48 | 14 |
| 11 | 92 | 90 | 90 | 97 | 90 | 99 | 8 | 11 |
| 12 | 99 | 105 | 98 | 128 | 87 | 108 | 7 | 14 |
| Sum | 1249 | 1266 | 1242 | 1259 | 1241 | 1240 | 179 | 300 |

On-minus-Off closing differences span −11 to +30 prey for the specialist and
−18 to +21 for the facultative. The mixed signs and near-equal pooled endings
are not proof of harmless predation: births, competition, movement and natural
death continue in the paired worlds. Total-prey time integrals were respectively
166890646, 166290168, 166418962, 166987916, 166057915 and 165529386 prey-ticks
in the table's arm order, each over 1728000 pooled world-ticks.

No arm's total prey fell below half its opening population. Form-specific
outcomes are less reassuring: form 1 fell below half in **6/12 specialist-on**
arms versus **0/12 specialist-off**, and **4/12 facultative-on** versus **1/12
facultative-off**. No form 0–2 reached zero. Form 3 (skimmer) was already absent
in eight openings; among the four present openings, new zero counts were
1, 2, 3, 2, 4, 2 seeds in arm order. This pre-existing diversity problem remains
open; form loss and opening-cohort extinction must not be conflated.

## Hunter outcomes and sparsity

Specialist-on had 179 captures, 210 misses, 463 out-of-reach settlements,
59 unaffordable refusals, one unmapped grasp and one lost target. Facultative-on
had 300 captures, 339 misses, 673 out-of-reach settlements, 22 unaffordable
refusals and one unmapped grasp. Unaffordable refusals are not paid strikes.
Eleven specialist-on seeds captured at least once; all twelve facultative-on did.

All **48 introduced founders**, including the attack-off controls, died of
starvation. Specialist-on survival ranged 533.5–3473.15 seconds; facultative-on
608.3–6906.1 seconds. None produced an observed child, adult descendant, or
second-generation parent. There is no self-replacing lineage here.

Pooled occupancy bins `[0, 1, 2, >2 adults]`:

```text
specialist off    [1530881, 197119, 0, 0]
specialist on     [1362373, 365627, 0, 0]
facultative off   [1524739, 203261, 0, 0]
facultative on    [1182818, 545182, 0, 0]
```

Each denominator is 1728000 world-ticks. Occupied time is entirely one imported
adult, followed by extinction. A nominal 100% of occupied time at one or two
adults therefore does **not** satisfy the user's reproducing-megafauna aspiration.

## Local occupancy recovery, with denominators

The fixed graph-radius-3, first-capture-per-600-second-bin rule selected 32 of
179 specialist captures and 48 of 300 facultative captures. Respectively 147
and 252 captures were unselected, but remain in the exposure stream.

| Selected-window status | Specialist | Facultative |
| --- | ---: | ---: |
| Recovered with full held confirmation | 30 | 41 |
| No measured deficit | 1 | 3 |
| Not recovered within the one-hour window | 1 | 2 |
| Right-censored at the run end | 0 | 2 |
| Total selected windows | 32 | 48 |

Specialist single-exposure windows: seven recovered. Its 25 windows with further
captures by that treatment in the same neighborhood: 23 recovered, one no deficit,
one not recovered within an hour. Facultative single-exposure windows: nine
recovered, one no deficit, two not recovered within an hour. Its 36 recurrent
windows: 32 recovered, two no deficit, two right-censored.

These are treatment-selected occupancy sites, not random samples or proof that
the killed organisms' lineages were replaced. One-hour non-recovery is censored
beyond that hour. Do not count no-deficit or censored windows as successes or
failures, and do not divide recoveries by all captures. Per-form exposure counts
exist; per-form local recovery windows do not.

## Next work

Keep this full cohort as the unsuccessful fixed-profile baseline. Finish the
reproduction observer hardening and real-world art review. Diagnose how food
acquisition, movement/sensing costs, digestion and the reproductive reserve/energy
gates interact before changing a documented parameter family. The
[seed-6 replay](hunter-opportunity-probe-2026-09-13.md) already shows a hunter with
96 captures that never reached either stock gate. Any revised profile must repeat
all prescribed seeds and controls; no selective placement or rescue.

The 24/72-hour ecology and diversity work remains open. This screen does not
establish long-run balance, paid lineage replacement, or physical-panel legibility.
The owning live cube remains on its frozen schema-9 build with no hunters.
