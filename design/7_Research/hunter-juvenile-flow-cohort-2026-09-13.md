---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Juvenile flow cohort: eleven arms, eighteen children, no growth step

All eleven birth-producing arms of the corrected charging cohort have been
replayed through the pinned mutation-site ledger. Across **161,636 gate
observation ticks** covering every one of the eighteen paid children, the growth
branch was entered **zero times** and no child gained any structure.

**This version corrects three factual errors and one overclaim in the previous
one.** Scavenging was reported as absent when eight children and seven adults
used it; the per-child "reserve in" column printed digestion rather than total
intake; the cohort was described as never rising above its birth escrow when four
children did; and the fixed-intake arithmetic was presented as a causal partition
of the cohort. Root's audit and the
[independent cohort review](astra-juvenile-cohort-review-2026-09-13.md) found all
four. The raw replays are unchanged and remain valid; only the reduction's
counting and the report's prose were wrong.

Read that review and the [gate review](hunter-juvenile-flow-independent-review-2026-09-13.md)
alongside this. Nothing here is a counterfactual, a parameter recommendation, or
authorization for a cohort, retune, migration or live change.

## What was run

Nine pinned replays, plus the two [earlier](hunter-juvenile-flow-diagnostic-2026-09-13.md)
ledgers reused rather than rerun. No replay was repeated for this correction.

| | |
| --- | --- |
| Executable | `captures/build-cache/juvenile-flow/release/examples/juvenile_flow` |
| Pinned SHA256 | `35943316416a947b6fedc42eaa973f787d2140f6fc53b95ca5360d6c52032977`, verified before reduction |
| Branch | `diagnostic/hunter-juvenile-flow-2026-09-13` at `0d867f1`, base `512ee52` |
| Inputs | the retained `captures/hunter-charge-candidate-two-hour-512ee52` arms, unchanged |
| Ledgers | `captures/juvenile-flow-cohort-2026-09-13/` (9) and `captures/juvenile-flow-2026-09-13/` (2), each pinned by SHA256 in the committed reduction |

No rebuild, no parameter change, no new seed, no care, no migration, no live
operation. Every arm ran the full 144000-tick horizon.

## What the reduction verifies, and what it only relays

The previous version claimed to "re-derive every gate". It did not: it compared
JSON labels with each other and trusted two booleans. The reduction now separates
the two honestly, and the distinction is carried in the output itself.

**Independently verified** — for all 11 arms, both retained `.cubw` files are
read and their CUBW envelope, schema, CRC32, FNV state hash and SHA256 are
recomputed from the bytes, then compared against `opening.json`, `summary.json`
*and* the replay's report. All 22 snapshots verified at schema 12 with matching
build labels. This is artifact evidence, not two records agreeing.

**Relayed as the replay's own claims** — observer neutrality and per-tick
flow/stock reconciliation. Reproducing those needs the replay. They are validated
for shape, sign, finiteness and against this file's own frozen `1e-9` tolerance
(never the tolerance a report states), and labelled `reported_by_replay`.

**Per-member evidence the ledger carries** is now actually checked, for all 29
members rather than the 18 children: every flow finite and non-negative, every
endpoint stock finite, per-member `violations` zero, every residual maximum
within the frozen tolerance, and the boundary identity holding individually. A
member violation hidden behind an aggregate zero is caught.

The cohort is a **frozen set of eleven arms**. A missing, duplicate, unexpected or
malformed arm is a failure, not a smaller cohort; statistics come only from arms
that passed everything, and the CLI exits non-zero on any coverage, cross-check or
boundary failure — not only on gate failures. Current run: `complete: true`,
11/11 valid arms, 18/18 children, 29/29 members, 0 invalid arms.

| Arm | independently verified snapshots | reported event records | reported violations |
| --- | --- | ---: | ---: |
| 1 specialist_on | opening + closing | 1135 | 0 |
| 1 facultative_on | opening + closing | 1130 | 0 |
| 2 facultative_on | opening + closing | 1221 | 0 |
| 5 specialist_on | opening + closing | 948 | 0 |
| 5 facultative_on | opening + closing | 942 | 0 |
| 6 facultative_on | opening + closing | 757 | 0 |
| 7 specialist_on | opening + closing | 935 | 0 |
| 7 facultative_on | opening + closing | 984 | 0 |
| 8 specialist_on | opening + closing | 997 | 0 |
| 8 facultative_on | opening + closing | 925 | 0 |
| 12 facultative_on | opening + closing | 1101 | 0 |

**Gate observations and reconciliation checks are different counts.** Over the
eighteen children: 161,636 gate observations against 161,654 reconciliation
checks. The 18 extra are one per child at its birth boundary — a mid-replay birth
is registered during the commit that follows the physiology pass, so its birth
tick is probed but not gate-observed. A death then replaces that tick's
end-of-tick probe with one removal-site check, which is why the two counts match
for a member already present when the ledger opened. Asserted per member; holds
for all 29. Worst reserve closure residual: **4.26e-13**.

### Two independent measurement paths agree, in both directions

Every ledger child must appear in the earlier census/transaction reduction and
every census child in the ledgers. **18 of 18 agree** on identity, birth tick,
death cause, censoring, lifespan and paid strike energy.

- The core emits its death event at `now + 1` while the ledger uses the
  pre-increment `now`, so a ledger `end_tick` sits one below the event-stream
  death tick. The convention-free check — ledger gate observations against the
  event stream's birth-to-death span — holds exactly for all 18.
- Seed-12 `35:8` shows 17 attempts against 5 paid strikes because **12 attempts
  were `Unaffordable`**: the child could not pay the 0.08 strike cost, so they
  never reached the charge site. Payable attempts equal paid strikes for all 18.

## Every child, by full ID

Reserve intake is shown **per source**. Frugivory and grazing are exactly zero for
every member; scavenging is not.

| Seed / arm | child | parent | born | end | ticks | digestion | scavenging | total intake | ox burn | max reserve | upkeep paid | paid strikes |
| --- | --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 S | `17:3` | `11:5` | 170401 | 179982 starved | 9582 | 0.5165 | 0.0000 | 0.5165 | 1.3165 | 0.7995 | 2.489 | 8 |
| 1 S | `73:5` | `11:5` | 208802 | 214410 starved | 5609 | 0.0000 | 0.0000 | 0.0000 | 0.8000 | 0.7995 | 1.560 | 4 |
| 1 S | `27:11` | `11:5` | 276004 | 283127 starved | 7124 | 0.0000 | 0.0000 | 0.0000 | 0.8000 | 0.7995 | 1.880 | 0 |
| 1 F | `17:3` | `11:5` | 170401 | 179982 starved | 9582 | 0.5165 | 0.0000 | 0.5165 | 1.3165 | 0.7995 | 2.489 | 8 |
| 1 F | `73:5` | `11:5` | 208802 | 214410 starved | 5609 | 0.0000 | 0.0000 | 0.0000 | 0.8000 | 0.7995 | 1.560 | 4 |
| 1 F | `27:11` | `11:5` | 276004 | 283153 starved | 7150 | 0.0000 | 0.0000 | 0.0000 | 0.8000 | 0.7995 | 1.880 | 0 |
| 2 F | `15:6` | `52:5` | 186151 | 197235 starved | 11085 | 0.4500 | 0.0083 | 0.4583 | 1.2583 | 0.7995 | 2.497 | 5 |
| 2 F | `86:7` | `52:5` | 224552 | 233784 starved | 9233 | 0.4693 | 0.0343 | 0.5035 | 1.3035 | **0.9897** | 2.527 | 7 |
| 2 F | `94:5` | `52:5` | 285107 | **censored alive** | 2893 | 0.4588 | 0.0291 | 0.4879 | 1.2196 | **1.0870** | 0.787 | 2 |
| 5 S | `76:5` | `9:4` | 170401 | 176277 starved | 5877 | 0.1624 | 0.0000 | 0.1624 | 0.9624 | 0.7995 | 1.600 | 8 |
| 5 F | `108:1` | `9:4` | 177536 | 186234 starved | 8699 | 0.0000 | **0.1827** | 0.1827 | 0.9827 | 0.7995 | 2.294 | 0 |
| 6 F | `74:5` | `29:6` | 170739 | 179397 starved | 8659 | 0.3037 | 0.0000 | 0.3037 | 1.1037 | **1.0272** | 2.396 | 2 |
| 7 S | `15:4` | `54:5` | 172294 | 176819 starved | 4526 | 0.0000 | 0.0000 | 0.0000 | 0.8000 | 0.7995 | 1.240 | 8 |
| 7 F | `86:7` | `54:5` | 172294 | 184194 starved | 11901 | 0.7364 | 0.0529 | 0.7893 | 1.5893 | 0.7995 | 3.168 | 7 |
| 8 S | `41:5` | `6:6` | 173805 | 196434 starved | 22630 | 2.7227 | 0.0000 | 2.7227 | 3.5227 | **0.8545** | 5.867 | 30 |
| 8 F | `53:6` | `6:6` | 171270 | 178000 starved | 6731 | 0.0000 | 0.0285 | 0.0285 | 0.8285 | 0.7995 | 1.705 | 3 |
| 12 F | `11:9` | `38:4` | 183923 | 197113 starved | 13191 | 1.1514 | 0.1661 | 1.3175 | 2.1175 | 0.7995 | 3.478 | 18 |
| 12 F | `35:8` | `38:4` | 222324 | 233878 starved | 11555 | 0.7204 | 0.0001 | 0.7205 | 1.5205 | 0.7995 | 3.075 | 5 |

End ticks are the ledger's pre-increment labels; add one for the event stream's.
Seed 1's two hunting arms produced children with the same IDs and near-identical
histories — separate worlds that track closely; `27:11` diverges by 26 ticks.

**Seventeen starved; one is censored alive** at the horizon after 2893 ticks, so
its totals are horizon-bounded rather than a completed lifetime.

## What the cohort establishes

**Growth never ran anywhere.** 0 of 18 children entered the branch, gained
structure, or reached adult structure. 0 of 18 ever had reserve above the 1.2
gate. The growth cap-attribution counters remain unexercised by artifact evidence.

**Reserve intake came from two sources, not one.**

| Reserve intake, 18 children | amount | share |
| --- | ---: | ---: |
| Digestion | 8.2082 | 94.24% |
| Scavenging | 0.5019 | 5.76% |
| Frugivory, grazing | 0.0000 | 0% |
| **Total** | **8.7101** | |

Eight children scavenged. **Seven children had zero digestion, but only five had
zero total intake**: seed-5 `108:1` and seed-8 `53:6` never struck or digested
prey and still acquired reserve by scavenging, so their zero-attempt histories
cannot be described as zero acquisition. Scavenging also supplied **0.3346**
battery energy. Seven of the eleven adult parents scavenged too, for 6.0615
reserve — not one adult, as previously stated.

**Reserve rose above the birth escrow in four children, and cleared the gate in
none.** Fourteen peaked at approximately 0.7995 — their 0.8 escrow minus a tick of
oxidation — but seed-2 `86:7` reached 0.9897, seed-6 `74:5` 1.0272, seed-8 `41:5`
0.8545 and seed-2 `94:5` **1.0870**. These are maxima at the ledger's
gate-observation site, not a claim to have seen every intra-tick assignment. The
supported statement is that **none reached 1.2**.

**All recorded child reserve outflow was oxidation**: 23.0418 out, growth 0.0000,
funding 0.0000.

**The above-reference classification separates adults from children sharply.**
Cohort-wide, **3.55%** of child reserve burn and 3.58% of child oxidation ticks
were recorded above the world's configured 0.5 reference; **fourteen children
recorded 0.0%**. The eleven adult parents recorded 92.6%–100%. This classifies
recorded states on realized paths. It is **not** a counterfactual: a different
threshold changes the adult's stocks, its funding and birth timing, prey
interactions and the child's own later encounters, and it excludes nothing about
adult or environment effects.

**Upkeep dominates the child energy account.** Of 52.372 energy paid out: upkeep
42.490 (81.1%), strikes 9.520 (18.2%), handling 0.362 (0.7%). Of 42.493 demanded
upkeep: sensing 19.396 (45.6%), maintenance 16.164 (38.0%), movement 6.933
(16.3%). The clamped payment is not divided across those terms anywhere in this
reduction; demand and payment are reported side by side. Payment fell short of
demand on 17 ticks in total.

### A bookkeeping ceiling, not a partition of the cohort

Reserve has one inflow set and, below the gate, one outflow. So under the explicit
condition that **each child's recorded intake and its timing are held fixed and no
other flow responds**, `escrow + recorded total intake` is a ceiling on what that
recorded intake could have supported:

| Under that fixed-intake condition | children |
| --- | ---: |
| Escrow + recorded total intake exceeds 1.2 | 9 |
| At or below 1.2 | 9 |

Computed from total intake including scavenging, as the review required. **This is
arithmetic over a frozen recorded sequence and nothing more.** It does not show
that nine children "fail on acquisition regardless of the sink", and it does not
show the sink "cannot be the whole answer". Removing or reducing a sink changes
battery level, lifespan, movement, strike affordability, encounter opportunities
and potentially the parent's own history, any of which changes intake itself —
and those trajectories were not observed. The censored child's total is also
horizon-bounded. The previous version's causal phrasing is withdrawn.

## What is still unmeasured

- **Encounter and approach.** The ledger counts paid strikes, digestion and
  scavenging ticks, and recorded `Unaffordable` refusals. It does not observe
  whether prey was within reach and not struck.
- **Any counterfactual.** No juvenile in this cohort ran under the world's
  configured threshold.
- **Dormant branches.** Reconciliation proves completeness only for stock
  movements a replay exercised.

## Next step: not another diagnostic loop yet

The previous version proposed a single in-envelope encounter counter as
"discriminating". **It is not, and that proposal is withdrawn.** Prey in the
envelope without a strike can mean cooldown, insufficient energy to pay for the
strike (12 recorded `Unaffordable` attempts show that is real), controller phase,
commitment to another target, or target choice; prey never in the envelope can
reflect a failed approach as easily as an absent opportunity. A bare
reachable-but-unstruck count cannot separate geometry from availability.

A design that could discriminate would have to be **state-conditioned**: an
opportunity counter qualified by the member's own eligibility at that tick —
phase, cooldown, target commitment and whether the strike cost was affordable —
so that "declined an eligible opportunity" is distinguishable from "was not in a
position to take one". That is a specification sketch, not a proposal to build,
and the direction is deliberately left open.

No further replay, instrumentation or tuning step is proposed here. The corrected
evidence above is the deliverable; the next biology experiment should be chosen
against it, not against another instrumentation package.

**No parameter change is authorized by this cohort.** An adult-scoped charging
threshold remains a future candidate at best, and this reduction does not
strengthen its case. No 24- or 72-hour hunter study follows from a passing
diagnostic.

## Verification and retained failures

Subsequent independent correction review **b1f1d65** repairs the JSON-only test
fixture by copying the actual retained snapshots into its temporary arm directories.
The original independent suite now passes **14/14**, including a byte-verified
positive control and intended-reason assertions on every CLI refusal. The native
18-test suite and fresh actual11-arm reduction also pass. See the
[follow-up review](juvenile-cohort-correction-independent-review-2026-09-13.md).
The13/1 result below records the earlier run, before that fixture repair; missing
snapshots still fail and no validation tolerance was weakened.

- `node --test scripts/reduce-juvenile-flow-cohort.test.mjs` — **18 passed, 0
  failed** (was 10; the new ones cover per-source intake, the zero-digestion
  scavenger, reserve above escrow, all-member gate coverage, byte-level snapshot
  checks, the frozen tolerance, both cross-check directions and the exit verdict).
- `node --test scripts/astra-juvenile-cohort-review.test.mjs` — **13 passed, 1
  failed**, from 2 passed / 12 failed against `02f84ce`. Every one of the
  independent reviewer's red cases now passes.
- **The one remaining failure is retained and is a fixture gap, not a reducer
  defect.** `CLI fixture control reproduces complete 11-arm/18-child acceptance`
  builds a JSON-only temporary cohort and deliberately copies no `.cubw` files.
  Byte-level snapshot verification now requires them, so all 11 fixture arms fail
  with gate `snapshot_unreadable` naming the exact missing path, and the CLI
  correctly refuses. Weakening that check to make the control pass would restore
  precisely the gap the review asked to close, so it has not been weakened; the
  fixture needs the snapshots (or an explicit reduced-evidence mode) on the
  reviewer's side. That file is theirs and was not edited.
- `node scripts/reduce-juvenile-flow-cohort.mjs …` — exit 0; `complete: true`,
  11/11 valid arms, 18/18 children, 29/29 members, 0 invalid arms, 0 boundary
  failures, 18 cross-checked with 0 disagreements. Committed reduction:
  [`assets/hunter-juvenile-flow-cohort-reduction-2026-09-13.json`](assets/hunter-juvenile-flow-cohort-reduction-2026-09-13.json),
  regenerated deterministically after the tests.
- No raw ledger, retained artifact, source copy or original failure record was
  overwritten or deleted. This is not a biological completion claim.
