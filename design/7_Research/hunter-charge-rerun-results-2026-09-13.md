---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Corrected charging rerun: technically complete, biologically still sterile

The twelve-seed paid-charging pair now completes. All 144 arms reach tick 288000,
the unchanged strict comparator passes, and an independent transaction/census
reduction reconciles with every arm's own audit. **That is a technical result, not
a lineage-viability result.** Charging again bought real, fully paid offspring —
18 of them, in complete arms this time — and again bought no adult: not one paid
descendant reached adult structure in any seed or arm, and none was ever observed
above its birth structure of 0.8. The
[earlier failed study](astra-hunter-charging-results-2026-09-13.md) and its
invalid artifacts remain untouched and are still the record of what happened at
`3b06596`. No ecology parameter, live world, renderer or canonical decision
changed here; Lore/Graft/canon and the charging proposal were consulted.

## Provenance

Build `0.1.0+512ee52`, frozen executable SHA256
`65d42d78ecc3fc6fff4e9f5490d5e78df61f7f275ee8584e45221c3177b02828` recorded and
verified in both manifests. Source worktree `/tmp/cubarium-charge-rerun-512ee52`
is clean at `512ee52a54207ff3d6b59b6fee753c9603480af6`. Same twelve schema-9
mature openings at tick 144000, six arms, 144000 ticks, audit window 200, no
care. The candidate differs from the background only in semantic profile version
3→4 — a fixed 0.80 `E_max` oxidation activation for authoritative members.

- `captures/hunter-charge-background-two-hour-512ee52` (`reserve-targets-v1`)
- `captures/hunter-charge-candidate-two-hour-512ee52` (`reserve-targets-charge80-v1`)

Root's strict run was reproduced here: `node scripts/compare-hunter-recipes.mjs
BG CAND charge80` exits 0 and its output is **byte-identical** to
`/tmp/cubarium-charge-two-hour-512ee52-comparison.json`. No gate was weakened.

## Two passes, kept apart

The strict comparator is the artifact gate. It checks manifests, executable
identity, snapshot envelopes/CRC/SHA/state hashes, recipe isolation to the
version field alone, summary coverage and the internal consistency of the
oxidation diagnostics. It does not reconstruct the biology.

The new [reducer](../../scripts/reduce-hunter-charge-rerun.mjs) imports that
comparator unchanged, requires it to pass, and then rebuilds the biology a second
time from raw records: every `hunter/reproduction` transaction followed by its own
funding key, every attempt/capture/death from `events.jsonl`, every member's
stocks from the 720 census boundaries per arm. It labels its quantities
`ledger_*` (exact transactions), `sampled_*` (200-tick boundaries) and `summary_*`
(the run's own tickwise audit) and never substitutes one for another. The
[compact reduction](assets/hunter-charge-rerun-reduction-2026-09-13.json) keeps
every seed, arm, prey channel, zero-capture child, null opportunity and
unsuccessful outcome.

Reconciliation result: **0 mismatches** across 144 arms on captures, offspring,
all eight reproduction counts, reproduction material and heat. Independent
technical audits: 144/144 arms pass with `failure: null`; worst peak/fixed-limit
ratio over the whole cohort is `0.010164` (water, seed-11 untouched background).
Whole-prey channel status is `no_decline` in all 144 arms.

Agreement here means the summary's arithmetic survives a second derivation from
its own records. It is not independent confirmation of a value only the summary
measures — the tickwise stock-times and blocker histograms have no second source.

## What the narrow fix actually changed

The repair claim was tested by closing state hash, not by reading the diff.

| Run | Arms reproduced bit-exact | Arms changed |
| --- | ---: | ---: |
| Background `3b06596` → `512ee52` | **72 / 72** | 0 |
| Candidate `3b06596` → `512ee52` | 66 / 72 | **6** |

The entire background cohort reproduced exactly. The candidate changed in seed 6
and nowhere else — the six arms that the old driver aborted at tick 170772. The
eleven previously valid candidate seeds are the *same worlds*, not merely similar
statistics. This is measured reproduction of the pre-existing trajectories; it is
not a general claim that a repaired active-hunter branch must be identical.

Seed 6, previously the failure, now runs to the horizon in all six arms. Its
facultative closing state was `9140998897574537509` — the exact invalid state the
old replay reproduced byte-for-byte — and is now `11174133049431148431` at
tick 288000.

| Seed-6 arm | prior (truncated 170772) | corrected (288000) |
| --- | --- | --- |
| untouched | 0 captures | 0 captures |
| budget_control | 0 captures | 0 captures |
| specialist_off | founder dead 160421 | founder dead 160421, unchanged |
| specialist_on | 17 captures, 1 hunter live | 18 captures, founder dead 177503, **0 hunters** |
| facultative_off | founder dead 162341 | founder dead 162341, unchanged |
| facultative_on | 18 captures, 1 birth, 2 hunters live | 21 captures, 1 birth, founder dead 176887, lineage extinct 179398, **0 hunters** |

The old report censored seed 6's two live hunting founders rather than calling
them survivors. That caution was correct: both died, and the paid child died at
432.95 s. Seed 6 contributes nothing to survival and one more starved juvenile.

## Every seed's hunting arms

Founder lifetime in minutes after the opening; `120+` is alive at the horizon, a
censoring, not a measured death.

| Seed | Specialist: captures BG→CAND; births; founder BG→CAND | Facultative: captures BG→CAND; births; founder BG→CAND |
| --- | --- | --- |
| 1 | 28→114; 3; 38.40→120+ | 68→114; 3; 86.97→120+ |
| 2 | 16→10; 0; 22.06→18.50 | 21→119; 3; 26.07→120+ |
| 3 | 15→26; 0; 25.14→29.04 | 90→24; 0; 91.14→28.78 |
| 4 | 26→12; 0; 33.89→16.07 | 15→8; 0; 17.57→13.32 |
| 5 | 58→33; 1; 70.02→35.92 | 85→44; 1; 94.50→54.47 |
| 6 | 25→18; 0; 31.00→27.92 | 25→21; 1; 31.00→27.41 |
| 7 | 65→32; 1; 77.57→36.32 | 61→58; 1; 79.14→58.35 |
| 8 | 26→47; 1; 29.01→33.04 | 25→28; 1; 28.60→28.56 |
| 9 | 3→4; 0; 8.66→9.44 | 3→6; 0; 9.30→13.01 |
| 10 | 24→57; 0; 27.23→60.35 | 75→19; 0; 83.33→27.54 |
| 11 | 11→12; 0; 18.92→22.17 | 11→13; 0; 18.92→22.30 |
| 12 | 34→53; 0; 41.85→51.68 | 30→82; 2; 42.16→76.14 |

Charging is not a survival improvement, it is a redistribution. Specialist
founders live longer in 7 seeds and shorter in 5; facultative founders live
longer in 5 and shorter in 7. The **120-minute restricted mean founder lifetime**
— each founder's observed life truncated at the shared 120-minute horizon, so a
survivor contributes exactly 120 and its actual lifetime is unknown and at least
120 — moves 35.31→38.37 minutes specialist and **50.73→49.16 facultative**. This
is a horizon-limited quantity, comparable across these arms only because they
share one horizon; it is not an estimate of mean lifetime, and a censored founder
is a founder still alive at 288000, not one that lived forever. The facultative
arm's restricted mean falls despite gaining two horizon survivors. Seeds 3, 5, 7
and 10 lose from half an hour to over an hour of facultative founder life.

Three founders survive to the horizon under charging (seed-1 S, seed-1 F,
seed-2 F) against **zero** in the entire background cohort. That is the one
unambiguous positive survival signal in this study.

Attack-off arms are not identity controls, and did not stay identical: mean
founder lifetime moves 13.690→13.689 minutes specialist-off and 14.116→14.014
facultative-off. Every off-arm founder still starves in every seed.

## Reproduction: everything paid, nothing recovered

Twelve-seed transaction ledger, rebuilt from the records rather than read off the
summary:

| Arm | funded | born | refunded | miscarried | open at horizon | not funded (cap / stocks) |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| specialist_on | 6 | 6 | 0 | 0 | 0 | 0 / 0 |
| facultative_on | 12 | 12 | 0 | 0 | 0 | 0 / 0 |
| all four other arms | 0 | 0 | 0 | 0 | 0 | 0 / 0 |

All 18 funded gestations closed as births. There were **no refunds, no
miscarriages, no open gestations, and no recorded unfunded refusals of either
kind** — the gate is never reached and declined, it is simply never reached.
Every funded record pays exactly 1.6 reserve and 1.0 battery from the parent's
own before/after pair (within 1e-12), escrows 0.8 structure / 0.8 reserve / 0.6
energy, books 0.4 construction heat; every birth books 1.6 birth heat. Zero
contract violations in 18 records.

**Adult descendants: 0. Descendants that reproduced: 0.** In all 144 arms.

Adult occupancy is tickwise from each arm's own audit, with no rarity forcing —
the `>2` column exists and is simply empty:

| Arm | occupancy 0 | 1 | 2 | >2 |
| --- | ---: | ---: | ---: | ---: |
| specialist_on background | 1219524 | 508476 | 0 | 0 |
| specialist_on candidate | 1175481 | 552519 | 0 | 0 |
| facultative_on background | 997565 | 730435 | 0 | 0 |
| facultative_on candidate | 1020159 | 707841 | 0 | 0 |
| specialist_off / facultative_off | ~1.53 M | ~0.20 M | 0 | 0 |
| untouched / budget_control | 1728000 | 0 | 0 | 0 |

`adult_max` is 1 in every arm of both runs. Two adults never coexist for one
tick anywhere in the cohort, so "rare adults" is not an available reading: the
occupied ticks are the founder alone, and the count returns to zero permanently
when it dies. Maximum live *organism* descendant depth reaches 7–12 in the
hunting arms, but that is the prey lineage; the hunter lineage never gets past
generation one.

## Prey: small, mixed, no collapse

Twelve-seed means over 12×144000 world-ticks, background→candidate.

| Arm | mean prey | closing prey | form-1 below-half ticks | newly-zero form channels |
| --- | --- | ---: | --- | --- |
| untouched | 96.580→96.580 | 1249→1249 | 0→0 | 3:form3 → 3:form3 |
| budget_control | 96.233→96.233 | 1266→1266 | 50815→50815 | 3,5:form3 → 3,5:form3 |
| specialist_off | 96.307→95.874 | 1242→1221 | 0→0 | 1,3,5:form3 → 3,10:form3 |
| specialist_on | 94.590→94.505 | 1218→1215 | 233676→54366 | 3,5:form3 → 1,3:form3 |
| facultative_off | 96.098→96.055 | 1241→1200 | 14024→0 | 1,3,5,10:form3 → 3:form3 |
| facultative_on | 94.349→94.153 | 1261→1217 | 243427→298474 | 3:form3 → 1:form3 |

Every newly-zero channel in the cohort is form 3, and the untouched control
already loses form 3 in seed 3 — these are not automatically hunter-caused
extinctions. Form-1 below-half exposure falls by 77% under specialist charging
and rises 23% under facultative charging: the direction is arm-specific and the
seed identities move, so the totals hide reshuffling rather than a clean effect.
Charging did not change geometry.

Treatment-selected local recovery after a capture, all twelve seeds:

| Status | specialist BG→CAND | facultative BG→CAND |
| --- | --- | --- |
| recovered | 23→28 | 44→36 |
| not recovered by 3600 s | 2→3 | 2→4 |
| right-censored | 0→4 | 1→5 |
| no measured deficit | 7→1 | 5→6 |
| insufficient pre-window | 11→11 | 11→11 |

These are selected captures, not a random sample, and censoring is not recovery.
No complete paired recovery pass is claimed.

## Charging cost: paid for, not free

Per-arm process-scoped totals over the realized candidate path, above the
background's reference activation. Not a counterfactual difference.

| Candidate arm | transactions | reserve burned | battery gained | conversion heat |
| --- | ---: | ---: | ---: | ---: |
| specialist_off | 48012 | 24.000 | 38.400 | 9.600 |
| specialist_on | 305982 | 152.794 | 244.470 | 61.118 |
| facultative_off | 49376 | 24.291 | 38.866 | 9.716 |
| facultative_on | 395870 | 197.223 | 315.557 | 78.889 |

Background charging transactions are 0 in all 72 arms, as the gate requires.
Burned reserve returns material to local nutrient and pays conversion loss.

## The juvenile bottleneck, from the real offspring histories

All 18 children, by full ID, with their whole recorded lives. 17 starved; one
(seed-2 facultative `94:5`) is right-censored alive at 144.65 s. Lifetimes span
144.65–1131.50 s, median 432.95 s.

| Seed / arm | child | parent | captures | attempts | material | meal energy | life (s) | max R | max E |
| --- | --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: |
| 1 S | 17:3 | 11:5 | 2 | 2C 4OoR 2M | 1.076 | 1.922 | 479.1 | 0.7005 | 1.523 |
| 1 S | 73:5 | 11:5 | 0 | 3OoR 1M | 0 | 0 | 280.4 | 0.7010 | 1.431 |
| 1 S | 27:11 | 11:5 | 0 | none | 0 | 0 | 356.2 | 0.7020 | 1.500 |
| 1 F | 17:3 | 11:5 | 2 | 2C 4OoR 2M | 1.076 | 1.922 | 479.1 | 0.7005 | 1.523 |
| 1 F | 73:5 | 11:5 | 0 | 3OoR 1M | 0 | 0 | 280.4 | 0.7010 | 1.431 |
| 1 F | 27:11 | 11:5 | 0 | none | 0 | 0 | 357.5 | 0.7020 | 1.507 |
| 2 F | 15:6 | 52:5 | 2 | 2C 3M | 1.076 | 1.500 | 554.3 | 0.7758 | 1.514 |
| 2 F | 86:7 | 52:5 | 2 | 2C 3M 2OoR | 0.829 | 1.728 | 461.6 | 0.9352 | 1.903 |
| 2 F | 94:5 | 52:5 | 2 | 2C | 1.016 | 1.929 | 144.7* | 1.0294 | 2.117 |
| 5 S | 76:5 | 9:4 | 1 | 1C 5OoR 2M | 0.400 | 0.541 | 293.9 | 0.7005 | 1.433 |
| 5 F | 108:1 | 9:4 | 0 | none | 0 | 0 | 434.9 | 0.7680 | 1.768 |
| 6 F | 74:5 | 29:6 | 1 | 1C 1OoR | 0.604 | 1.012 | 432.9 | 0.9732 | 1.792 |
| 7 S | 15:4 | 54:5 | 0 | 7OoR 1M | 0 | 0 | 226.3 | 0.7470 | 1.334 |
| 7 F | 86:7 | 54:5 | 4 | 4C 2OoR 1M | 1.745 | 2.642 | 595.1 | 0.7470 | 2.037 |
| 8 S | 41:5 | 6:6 | 13 | 13C 11M 6OoR | 5.928 | 9.744 | 1131.5 | 0.8470 | 2.216 |
| 8 F | 53:6 | 6:6 | 0 | 3OoR | 0 | 0 | 336.6 | 0.7612 | 1.227 |
| 12 F | 11:9 | 38:4 | 5 | 5C 7OoR 6M | 2.487 | 4.040 | 659.6 | 0.7615 | 2.226 |
| 12 F | 35:8 | 38:4 | 4 | 4C 1M 12 Unaffordable | 1.926 | 2.401 | 577.8 | 0.7620 | 1.622 |

\* right-censored alive. C = Captured, M = Missed, OoR = OutOfReach.

**Growth produced nothing through each child's last observed census.** Across 806
census boundaries covering all 18 children, the set of distinct structure values
observed is exactly `{0.8}`. In the frozen source, `world.rs:1694`
(`o.structure += grown`) is the *only* site that raises an organism's structure,
and no site lowers it, so structure is monotone. Monotonicity plus a constant 0.8
at every boundary therefore rules out a growth spike that the census merely
missed *between* boundaries — but only over the span the boundaries bracket.

It does **not** cover the terminal suffix between each child's last census and
its death, which no observation reaches:

| | ticks | seconds |
| --- | ---: | ---: |
| Total unobserved terminal suffix, 18 children | 1646 | 82.30 |
| Longest single suffix (seed-6 facultative `74:5`) | 198 | 9.90 |
| Share of the children's 161636 observed life-ticks | 1.02% | |

Two separate things are known about that suffix and they should not be merged.
The tickwise `adult_occupancy` audit is a whole-run quantity, so **no child became
an adult** in the suffix either; and the growth branch is rate-capped at
`juvenile_growth_rate × DT`, so any unobserved gain is bounded above by 0.0199
structure in the longest suffix, against the 1.2 a child needs. What is *not*
known is whether the branch ran at all in those final ticks, or how much reserve
it would have found. Both are magnitude bounds and an absence-of-adults result,
not evidence that growth was zero. That question stays open until the
mutation-site ledger measures it directly; the diagnostic recommended below is
required to include the last-census-to-death suffix in its growth totals for
exactly this reason.

The previous report's "sampled child reserve maxima never exceed 1.02943" was
correctly hedged, and the structure record above narrows the same conclusion
without leaning on the reserve threshold — over the bracketed span only.

**Where the evidence points, by stage:**

- *Encounter and approach*: **not excluded as limiting.** The three children that
  never attempted an attack held a target for most of their lives: 24 of 35
  boundaries in `Stalking` (seed-1 `27:11`, both hunting arms) and 31 of 44
  (seed-5 facultative `108:1`), the rest `Perched`. That establishes only that
  targets are *acquired* — acquisition is not successful intercept. A member that
  stalks for hundreds of seconds and never once reaches windup range is equally
  consistent with approach being the stage that fails, and nothing here
  distinguishes acquisition-then-failed-closure from a shortage of reachable
  encounters. No stage is ruled out by this observation.
- *Capture*: partly limiting. 7 of 18 children never captured anything;
  `OutOfReach` and `Missed` dominate their attempt records. A juvenile is at
  `body_scale = (0.8/2.0)^0.5 ≈ 0.632`, so its capture effector closes far nearer
  the root than an adult's while prey escape speed is unchanged. This is a
  plausible reading of the geometry, not a measured cause.
- *Intake*: happens, and is not sufficient. The seed-8 specialist child made 13
  captures for 5.928 material and 9.744 chemical energy, held gut material and
  energy repeatedly, and still starved — with structure at 0.8 throughout.
  "They never catch food" is refuted for that child.
- *Growth allocation*: this is where it breaks. `world.rs:1678` gates growth on
  `reserve > growth_reserve_min × reserve_max`. `reserve_max` is the phenotype's
  4.0 and is **not** scaled down for a juvenile, so the gate is a flat 1.2 for a
  child born with a 0.8 reserve escrow. Not one of 806 boundaries observes a
  child above 1.2; 573 of 806 observe reserve at exactly 0.
- *Energy charging*: it competes directly with growth for the same stock. The
  oxidation branch at `world.rs:1652` runs immediately before the growth branch
  in the same tick and burns reserve whenever `energy < threshold × energy_max`
  and `reserve > 0`. Under the candidate's 0.8 threshold that condition means
  `energy < 3.2`, and **no child sample in the cohort ever reached 3.2** — 0 of
  806. The battery mechanism the study installed converts precisely the stock the
  growth gate requires.

The budget, from each child's own recorded meals against the configured
oxidation rate (`0.01 s⁻¹`), with digestion read at its most generous —
`eta_m · min(rho/e_r, 1) · material` at full assimilation with reserve headroom:

| | value |
| --- | ---: |
| Reserve supply, 18 children (0.8 escrow each + assimilable meals) | 22.660 |
| Oxidation demand over their 8081.8 s of observed life | 80.818 |
| Supply / demand | **0.280** |

Per child the ratio runs 0.184–0.906, and the only child above 0.5 is the one
still alive at 144.65 s. Actual burn is bounded by the reserve present, so the
deficit does not produce a negative stock — it produces a member pinned at zero,
which is exactly the 573/806 observation. A juvenile is born 0.4 reserve short of
its own growth gate and is net-negative on reserve from its first tick.

**What this does not establish.** It does not prove the raised threshold caused
the sterility. The background cohort produced zero births, so there is no
juvenile anywhere that ran under the world's configured 0.5 threshold, and no
counterfactual is available. On the same 806 samples, energy clears the
*background* shutoff (2.0) only 22 times — 2.7% — so a background-policy juvenile
would have had the oxidation branch active in the overwhelming majority of
observed states too. The threshold raise widens an already near-total burn
window; calling it the cause would be a mutation-site claim the data does not
support. The genuinely missing measurement is a tickwise per-member reserve flow:
how much each juvenile's reserve lost to oxidation versus maintenance, how much
digestion returned, and whether the growth predicate was ever near its gate. The
budget above is a bound from recorded transactions, not that ledger.

## Next action: one read-only flow observer, no cohort, no retune

**Built and run.** See
[the juvenile flow diagnostic](hunter-juvenile-flow-diagnostic-2026-09-13.md):
on the two arms measured so far the growth branch is confirmed never to have run,
including the terminal suffix this report could not observe, and the raised
charging threshold turns out to account for 12.0% and 0.0% of those juveniles'
reserve burn. The paragraph below is the proposal as written; the diagnostic's
findings supersede its expectation about what the ledger would show.

Add a **per-member tickwise reserve-flow observer** and replay the retained
candidate arms that actually produced children — `specialist_on` seeds 1, 5, 7, 8 and `facultative_on` seeds 1, 2, 5, 6, 7, 8,
12, eleven arms in all — from their own recorded
`post-initialization.cubw` under the frozen `512ee52` core, in the same pattern
the earlier failure replay used. Per member per tick it records the four reserve
flows separately: oxidation burn, digestion `to_reserve`, growth spend, and
maintenance; plus the growth predicate's two sides, read *before* the growth
branch rather than reconstructed from a post-step delta. Its growth totals must
run to each child's actual death tick, closing the 1646-tick terminal suffix this
report cannot observe. Its correctness gate is that each replayed arm reproduces
its recorded post-initialization input and closing state hash exactly, so the
observer is proven to have changed nothing.

This is the isolated diagnostic, and it is deliberately not an experiment: no
configuration, threshold, cost, geometry, gate or default moves; resource
accounting, ordinary organism defaults and the predator/prey guards stay intact;
nothing is granted free growth, free maintenance or free reproduction; no live
world is seeded. It needs no new twelve-seed cohort — the inputs are already on
disk and deterministic.

Only if that ledger shows oxidation dominating the juvenile reserve deficit does
the obvious follow-on become justified: a semantic profile version 5 that is
version 4 with the fixed member threshold scoped to adults, run as its own paired
arm. That is a separate proposal and is **not** recommended now; it is named only
so the scope of this recommendation stays clear. Nothing here bundles a meal,
geometry, offspring-cost or threshold change.

**Do not start a 24- or 72-hour hunter run on the strength of these audits.**
The technical gate passing does not meet a lineage-viability bar, and the bar is
unmet by a wide margin: zero adult descendants, zero descendant reproduction,
never two coexisting adults for a single tick in 144 arms, and the founder
lineage extinct in 21 of the candidate's 24 hunting arms and in all 24 of the
background's.

## Verification

- `node scripts/compare-hunter-recipes.mjs captures/hunter-charge-background-two-hour-512ee52
  captures/hunter-charge-candidate-two-hour-512ee52 charge80` — exit 0, output
  byte-identical to root's `/tmp/cubarium-charge-two-hour-512ee52-comparison.json`.
- `node scripts/reduce-hunter-charge-rerun.mjs BG CAND BG_3b06596 CAND_3b06596 [--compact]` —
  exit 0; 0 summary-vs-independent mismatches over 144 arms; 0 funding-contract
  violations over 18 records.
- `node --test scripts/reduce-hunter-charge-rerun.test.mjs` — 10 passed.
- `node --test scripts/compare-hunter-recipes.test.mjs` — 11 passed (unchanged).
- `node --test scripts/prepare-hunter-worlds.test.mjs` — 6 passed (unchanged).

No executable was run, no world was advanced, no core, host or art file was
touched, and no live operation was performed for this analysis.
