---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Ordinary quiet habits: reproduction takes the stock before satiation

**Measured result: zero active-to-Resting transitions in every seed and arm.**
Every observed Resting episode was a newborn's single initial tick, not a meal
followed by a quiet habit. This confirms and explains the near-zero Resting seen
in the earlier local-care screen. It is a diagnosis of the current biology, not
permission to fake resting in art or a canonical parameter choice.

## Frozen experiment and verification

Tool commit **a89179a**, `crates/cubarium/examples/astra_quiet_diagnosis.rs` and
its `observer.rs`. Ten tests passed: three observer tests, one short paired
audit/neutrality test, and six reused inventory tests. Strict hunger hysteresis,
newborn one-tick Resting, skipped/repeated-tick refusal and full-state equality
with an unobserved baseline are covered.

Actual release executable built from detached source at that exact commit:
`0.1.0+a89179a`, SHA256
`cf82eecc00d5b7702cc3c2c01f9638cb8e2e2f9d2b263a360f50a84c4d1e040d`.
It is retained as `captures/astra-quiet-a89179a/astra_quiet_diagnosis.frozen`.
Large build/output files stayed on the workspace filesystem, not the full tmpfs.

All twelve prescribed schema-9, tick-144000 mature openings were checksum-checked
and cloned into untreated and Feed arms. Each ran exactly **12000 ticks / ten
minutes**, ending at tick 156000. Feed was one Standard command at elapsed 600,
Front (32,48), the same fixed interior target for all seeds. This is a mechanism
follow-up, not a new all-target screen or a selected responsive placement.
No hunter, active shower, altered phenotype/config, ambient setting or core patch.

The actual foreground run finished **exit 0; all 24 arms passed** invariant,
material, corrected persisted energy, water, independent short-window energy
and immediate Feed boundary checks at unchanged opening-scaled tolerances.
Maximum residuals across all arms were respectively 1.94e-11, 9.78e-12,
1.23e-10 and 5.73e-11 (boundary checks also passed). Observer code additionally
verified the exact next hunger-memory bits and resulting Resting/non-Resting
decision for **every surviving existing ID on every step**, from that ID's
previous stocks and actual decoded thresholds. This is not a guessed gate from
a ten-second sample.

Artifacts retain all full slot+generation IDs, all birth/death events, observed
escrow transitions, every Resting episode with entry reason and censoring, and
all live organisms' stock/threshold trajectories every 200 ticks. Counts are
tickwise, including intervals between samples. There is a hard 20000-ID bound
that fails explicitly rather than dropping individuals; actual counts stayed
below 140 per arm. Failed arms would remain explicit in the terminal summary.

## The gate conflict is concrete

All observed individuals' decoded thresholds were the same: seek-off
0.10000000149, seek-on 0.30000001192, bud-reserve 0.69999998808 and bud-energy
0.30000001192; hunger-memory time constant 10 s. Ordinary hunger is
`clamp(1 - R/Rmax, 0, 1)`. Active fauna enter Resting only when updated hunger
memory is **below seek-off**. Consequently, sustained reserve near/above 90%
would be needed to make memory approach that rest threshold from above.

| Measurement, all twelve seeds pooled | Untreated | One local Feed |
| --- | ---: | ---: |
| Observed full IDs (including births) | 1522 | 1518 |
| Existing IDs at opening | 1105 | 1105 |
| Mean R/Rmax, organism-time weighted | 0.15862 | 0.16052 |
| Maximum observed R/Rmax | 0.7007103 | 0.7007015 |
| Minimum hunger memory | 0.3073869 | 0.3060949 |
| Instant hunger or memory below seek-off, ticks | 0 | 0 |
| Active→Resting entries | 0 | 0 |
| Resting episodes / ticks | 417 / 417 | 413 / 413 |
| Observed escrow starts | 403 | 407 |
| Births | 417 | 413 |

**No individual ever exceeded its instantaneous rest reserve threshold.** Every
observed member-tick still had memory above seek-on. This is not merely a slow
memory response that would obviously resolve after waiting another few seconds.

At all 403/407 observed new escrows, the previous completed reserve fraction was
**0.700002–0.700711**, just at the reproduction gate. The next completed fraction
was **0.0170–0.1717 untreated / 0.0102–0.1660 Feed**. Previous hunger memory was
0.307–0.433 / 0.306–0.433, nowhere near the .1 rest gate. Observed post-step
reserve-at-bud-gate ticks totaled exactly 403/407 too. This strongly identifies
**paid reproduction around .7 reserve as a repeated interruption of stock
accumulation before the approximately .9 satiation condition**, in this cohort.

The source explains the ordering: `controller.rs::decide` computes hunger memory
and `bud` from the pre-movement organism. In `world.rs` physiology, ordinary
oxidation and growth precede budding. Eligible parents pay child structure plus
child reserve from R and child energy plus building from E, then hold an escrow.
The ordinary `.7` budding gate is independent of `.1` seek-off. Births begin in
Resting, with hungry initial stores, and leave that mode on their next decision.
The exact gate checks and recorded episodes confirm this last behavior here.

The escrow observations are **not** new exact mutation-site transaction events:
the ordinary API publishes births/deaths, while detailed funding events are
hunter-only. Before/after points also include intake, oxidation and growth.
Therefore the table identifies timing and stock reset, not a reconstructed exact
reserve debit. Same-tick funding/death can be absent from the surviving-state
observer. Some opening escrows also finish in this window, so births need not
equal newly observed funding starts.

## Most organisms are also genuinely low-stock

Untreated fauna were at R=0 for **34.17% of organism-time**, and at or below the
.3 growth-reserve gate for **82.16%**. Of actual-fed ticks, **22.11% still ended
at R=0**, and 36.55% did not increase R over the previous completed state. Feed
changed those to 33.66%, 82.13%, 21.55% and 36.28%. These do not mean no food was
eaten: `fed` is true only after actual field intake; other processes spend stores.

Same-ID structure gains sum to **193.739 / 199.212 material**; observed newly
held escrow material totals **235.307 / 237.429** across these separate worlds.
Both are substantial stock uses. Immature bodies (structure below adult target,
not the renderer's juvenile cutoff) account for 45.69% / 46.12% of organism-time.

Post-step energy was below the configured .5 oxidation threshold for 41.15% /
40.65% of time. Source oxidation burns at most .01 reserve/second when its actual
phase-local energy gate is open, credits .8 of released energy, and exports the
burned material to nutrient. The observer **cannot quantify actual oxidation
from this post-step percentage or a reserve difference**: feeding/payment already
occurred before the branch and growth/reproduction occur afterwards. No exact
per-organism burn is claimed. Intake also has diet rates, local energy quality,
Type-II saturation, sharing and headroom limits; fed duration is not net storage.

## All seeds and forms, including nonresponses

Every seed has zero active-to-Resting entries in both arms. Counts below are
newborn single-tick Resting episodes, exactly equal to births in that arm.

| Seed | Untreated | Feed |
| --- | ---: | ---: |
| 1 | 33 | 28 |
| 2 | 40 | 37 |
| 3 | 30 | 32 |
| 4 | 38 | 39 |
| 5 | 31 | 31 |
| 6 | 40 | 39 |
| 7 | 32 | 34 |
| 8 | 22 | 24 |
| 9 | 41 | 41 |
| 10 | 43 | 41 |
| 11 | 23 | 23 |
| 12 | 44 | 44 |

| Form index | Observed IDs untreated / Feed | Mean reserve fraction untreated / Feed | Actual-fed % of organism-time untreated / Feed |
| --- | --- | --- | --- |
| 0 | 692 / 686 | .14797 / .15041 | 34.22 / 34.27 |
| 1 | 454 / 450 | .14357 / .14560 | 33.69 / 33.82 |
| 2 | 344 / 349 | .19363 / .19396 | 84.51 / 84.97 |
| 3 | 32 / 33 | .14876 / .15136 | 36.17 / 36.42 |
| 4–7 | 0 / 0 each | null | null |

Even form 2, eating during approximately 85% of its organism-time, never reaches
the rest stock condition. All 1105 opening individuals ate at least once. One
newborn in each arm never fed before the horizon, retained rather than excluded:
seed 2 baseline slot26/generation7 (262 observed ticks), Feed slot9/generation9
(1532 ticks). Both are right-censored alive, not reported as deaths.

Feed's world-wide fed fraction changes only 47.16→47.47%, unlike the stronger
target-local change in the previous screen; those are different denominators and
horizons. Starvation deaths are 327 untreated versus 333 Feed, with 14 age deaths
in each; births 417 versus 413. These mixed, short-run outcomes do not establish
Feed as a global survival/reproduction benefit and should not be hidden to support
a satisfying interaction story.

## Consequence for the next slice

The real-intake presentation prototype remains a useful way to show eating.
**It cannot supply the missing quiet biology.** No current observation supports
an authored post-meal sleeping/satiated pose for active fauna.

The next biological design question is the **ordering of reserve allocation and
rest**, not a global animation multiplier or a larger food dose. A bounded future
candidate should explicitly decide whether a parent sometimes pauses before
funding, or whether a paid short recovery habit is distinct from full satiation;
retain all offspring costs and resource accounting. Do not silently lower costs,
disable reproduction, relax hunger gates or change autonomous defaults on the
basis of this diagnosis. Those alternatives need an isolated behavior experiment,
not an art-only “fix.” No candidate parameter is selected here.

Read-only reducer with per-seed/form counters, complete event summaries and frozen
binary verification:

```sh
node design/7_Research/astra-quiet-reduce-2026-09-13.mjs
```

The command emits JSON and changes nothing. No motion-path metric or exact
settlement/burn ledger was added; the source-backed limitations above remain.
Only the isolated diagnostic, its tests and this research work were changed.
