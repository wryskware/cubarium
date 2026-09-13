---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Single-pulse care: feeding responds; sustained quiet is not demonstrated

Independent reduction of `captures/care-response-single-pulse-4d10351`, using
`astra-care-response-reduce-2026-09-13.mjs`. All **108 comparisons** are retained:
12 prescribed two-hour-old openings × three fixed targets × Feed/Rain/Clean.
Each comparison has a matched untreated arm, 30 s before one Standard pulse and
120 s afterwards. Ambient settings are unchanged. This is short-run evidence,
not a care-dependent-world prescription or a canonical biological decision.

The reducer checked manifest/summary completion, all expected comparison keys,
frozen executable and all opening SHA256s, 3000-tick reports, one correct receipt
at elapsed 600, receipt-versus-delivered care ledgers, sample cadence/count, pre-pulse equality, identical fixed cohorts,
and complete equality of the nine untreated copies for every seed. Both numerical
audit flags pass in every comparison. This checks retained audit results, not a
second reconstruction of core transactions. Manifest SHA256:
`2305d14dc56187dfdbe7eac1ffade954ddacdac1f3e2f9f208df2fd2e4b1d49e`.

## Main result

All counts below cover **post-pulse ticks 601–3000**. Dynamic local cumulative
counts subtract the elapsed-600 sample. Fixed-cohort counts start at that boundary
and follow the exact opening IDs anywhere; newcomers/descendants cannot replace
them. Counts are organism-ticks: divide by 20 for organism-seconds. They measure
any actual field intake, **not** manual-crumb ingestion or food mass consumed.

| Action | Local fed change | Local cases up / down / same | Fixed-ID fed change | Fixed cases up / down / same | Local occupancy change |
| --- | ---: | --- | ---: | --- | ---: |
| Feed | +39,245 | 27 / 7 / 2 | +20,535 | 21 / 8 / 7 | +20,864 |
| Rain | −665 | 14 / 16 / 6 | +1,018 | 12 / 14 / 10 | +1,535 |
| Clean | −12,051 | 9 / 25 / 2 | −8,716 | 8 / 21 / 7 | −3,094 |

Feed's signal is not only more animals spending time in the observation region.
Exposure-weighted fed fractions change from **64.17% to 77.67% locally**, and
**65.67% to 78.00% for fixed IDs**. Rain gives 63.21% / 66.12%; Clean gives
58.16% / 60.20%. These are pooled descriptive fractions, not independent-trial
confidence estimates. Targets share seed histories; repeated controls are not
additional independent worlds.

Among nonempty fixed cohorts, Feed raises fed fraction in 21/29 and lowers it in
8/29. For dynamic local fractions, Feed is up/down/same in 23/8/1 comparable
cases; four comparisons have a zero denominator in at least one arm and are
excluded from that fraction comparison, **not** from the count table. Baseline
has no post-pulse local occupancy at seed 9 target 1 and seed 11 target 1; Feed
also has none at seed 10 targets 0 and 1. Empty at pulse time is not the same as
empty throughout: arrivals can still change local counts.

All actions share seven empty pre-pulse cohorts: (seed,target) = (3,0), (5,1),
(7,0), (9,1), (10,0), (10,1), (11,1). Their fixed count differences stay zero and
fixed fractions are null. There are 69 opening memberships across the 36 target
cases; these are not necessarily 69 distinct individuals because regions can
overlap. Fixed living-time is 163,951 ticks untreated versus 164,361 in each
action's pooled arms; closing fixed losses are two untreated versus one cared.
Do not turn this tiny, censored survival difference into a longevity claim.

Feed increases summed local and fixed feeding in 11/12 seeds; seed 8 is negative
(−98 / −20 ticks). It also has positive pooled changes in each target class:
interior +15,548 / +8,004; seam +14,028 / +9,180; rim +9,669 / +3,351
(local / fixed). Individual negatives remain below. Seed 4 supplies 25.2% of the
pooled local gain, so the mean alone would hide substantial heterogeneity.

At one second, Feed already raises cumulative feeding in 9/36 cases, with no
decreases; at three seconds 11/36; at 30 seconds 19 up, four down, 13 unchanged.
Thus a prompt **local** meal response exists, but a mandatory immediate flourish
at every placement is not supported. Rain changes no feeding count at one
second; Clean decreases feeding in eight cases by then.

## Feeding is real; resting is almost absent

Untreated local feeding-mode time is 109,524 ticks, actual-fed time 109,520.
Feed gives 148,769 versus 148,765. The mode/intake mismatch is only four ticks in
each pooled set; Rain/Clean differ by five/four. Earlier source review correctly
identified that Feeding mode can occur without intake, but **this cohort does
not establish false Feeding as an important visual problem**.

Conversely, dynamic local Resting totals are **6 ticks untreated, 13 Feed, 6 Rain,
6 Clean** over all 36 target cases: 0.30–0.65 organism-seconds, not settled habits.
Every fixed cohort has **zero Resting ticks in every arm**. Extra Feed local rest
is seven single ticks spread over seven comparisons; even without a bout metric
that cannot demonstrate a sustained rest. Seeking time falls by 18,388 local /
20,125 fixed ticks with Feed, but Feeding remains an active mode. Slower feeding
locomotion must not be relabeled biological satiation or restful recovery.

## Rain and cleanup are different interventions

All 36 Feed receipts are Applied: 108 total material and 216 chemical energy
across these separate worlds. All Rain receipts are Applied, scheduling 144 total
depth; actual delivered ledger deltas sum to 144.00000000000003. At 120 s, each rain-treated region still has more water than its control
(+0.0196 to +1.1080 depth summed over that region); producer is higher in 26 cases
and lower in ten. No flood-time measurement identifies the cause of any decrease.
Do not demand an animal-feeding flourish as rain's success criterion.

All 36 Clean receipts are **Partial**, not rejected: the per-cell half-stock rule
limits real exports. They remove 27.2073 material and 24.2755 energy in total,
0.2090–1.6760 material per command against the nominal maximum 2. Closing litter
is lower in every targeted region. Most feeding contrasts are negative, but
positive cases exist (especially seed 8's rim), so neither guaranteed harm nor
guaranteed benefit follows. There is no measured toxicity or comfort response.
Closing litter/producer differences include reactions and later behavior, not
only the input transaction. Totals here pool separate comparisons, not one world.

## Every seed and target

Cells show **local / fixed-ID fed-tick differences**, cared minus untreated over
120 s. Target 0 = Front interior (32,48); 1 = Front seam (63.5,48);
2 = Right open rim (32,63.5). N is the pre-pulse local cohort size.

| Seed | Target | N | Feed Δ | Rain Δ | Clean Δ |
| --- | --- | ---: | ---: | ---: | ---: |
| 1 | 0 | 2 | 3313 / 1484 | 412 / -118 | -285 / -189 |
| 1 | 1 | 3 | 2649 / 791 | 125 / 24 | -520 / -12 |
| 1 | 2 | 4 | 91 / -47 | -293 / -329 | -1498 / -1186 |
| 2 | 0 | 2 | 2104 / 1523 | 20 / 111 | 411 / 270 |
| 2 | 1 | 1 | 308 / 308 | 1 / 1 | -474 / -474 |
| 2 | 2 | 1 | 24 / 18 | 36 / 0 | -952 / -749 |
| 3 | 0 | 0 | 35 / 0 | 0 / 0 | -70 / 0 |
| 3 | 1 | 1 | 943 / 129 | -252 / 151 | -147 / -148 |
| 3 | 2 | 3 | 2106 / 1020 | -42 / -42 | -2451 / -2451 |
| 4 | 0 | 3 | 2167 / 1242 | -95 / -5 | -408 / -543 |
| 4 | 1 | 5 | 7833 / 5674 | -403 / 632 | 112 / 812 |
| 4 | 2 | 3 | -115 / -1413 | -77 / -77 | -550 / -550 |
| 5 | 0 | 1 | 1340 / 50 | 34 / 38 | 55 / -39 |
| 5 | 1 | 0 | 583 / 0 | -4 / 0 | -210 / 0 |
| 5 | 2 | 2 | 439 / 442 | -70 / -41 | -365 / -215 |
| 6 | 0 | 3 | 2063 / 2017 | -317 / -1 | -65 / -120 |
| 6 | 1 | 2 | 112 / -193 | -4 / -63 | -130 / -4 |
| 6 | 2 | 4 | 78 / 78 | -329 / -329 | -701 / -701 |
| 7 | 0 | 0 | 846 / 0 | 21 / 0 | -172 / 0 |
| 7 | 1 | 1 | 406 / 320 | 16 / 1 | -100 / 110 |
| 7 | 2 | 2 | 1952 / 797 | 0 / 0 | -966 / -966 |
| 8 | 0 | 1 | -361 / 62 | -775 / -67 | -370 / 14 |
| 8 | 1 | 1 | 282 / -62 | 0 / 1 | 9 / 13 |
| 8 | 2 | 2 | -19 / -20 | 86 / 85 | 772 / 772 |
| 9 | 0 | 3 | 1060 / 892 | -116 / 161 | -844 / -860 |
| 9 | 1 | 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| 9 | 2 | 4 | 555 / -38 | -32 / -33 | -895 / -1213 |
| 10 | 0 | 0 | -271 / 0 | -36 / 0 | 125 / 0 |
| 10 | 1 | 0 | -762 / 0 | -35 / 0 | 19 / 0 |
| 10 | 2 | 2 | 2667 / 1536 | 1094 / 1067 | -413 / -415 |
| 11 | 0 | 1 | 3271 / 887 | 3 / -49 | -203 / -246 |
| 11 | 1 | 0 | 0 / 0 | 0 / 0 | 0 / 0 |
| 11 | 2 | 3 | 1932 / 1019 | 288 / 159 | -880 / -14 |
| 12 | 0 | 1 | -19 / -153 | 2 / -10 | 8 / -159 |
| 12 | 1 | 5 | 1674 / 2213 | 77 / -249 | -350 / 90 |
| 12 | 2 | 3 | -41 / -41 | 0 / 0 | 457 / 457 |

## Recommendation and limits

**A presentation-only actual-fed meal-onset prototype is justified**, because
there is existing local biology to express. This screen does not prove the
current art is unreadable: compare the same recorded response with/without a
subtle intake-driven bite/handling gesture, retaining continuous individual meal
loops and ordinary movement. Triggering on actual intake remains truthful even
though Feeding happens to be a close proxy here. Do not reset the pose every fed
tick, invent satiated rest, change controllers or force distant animals to gather.
Respect real gestation and later real Resting transitions; do not promise a
post-meal resting scene from these data.

For **quiet habits**, first diagnose ordinary reserve/hunger-memory and
reproduction/growth/oxidation trajectories around intake, together with actual
Resting entries/durations, in copied mature worlds. That bounded follow-up should
explain why rest thresholds are almost never crossed before proposing one paid,
resource-conserving behavioral change. This is separate from a visual bite and
from independently tuned ambient support. It must not lower autonomous support
or change hunger thresholds merely to force an interaction flourish.

Missing motion paths, bout durations, edible-D, per-diet/amount intake and
flood-time are explicit limits. No causally identified consumption of manual
crumbs, long-run recovery, room-distance readability, or new survival requirement
is claimed. No source/world/controller/art/live change was made by this analysis.

Reproduce the detailed per-row, per-seed and per-target reduction (JSON stdout):

```sh
node design/7_Research/astra-care-response-reduce-2026-09-13.mjs
```

The script is read-only. It retains every comparison and raw-file SHA256; null
fractions remain null. Its summaries are reductions of the checked raw artifacts,
not replacements for the frozen numerical audits or a new experiment run.
