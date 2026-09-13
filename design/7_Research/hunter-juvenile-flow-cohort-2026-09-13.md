---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Juvenile flow cohort: eleven arms, eighteen children, no growth step

All eleven birth-producing arms of the corrected charging cohort have now been
replayed through the pinned mutation-site ledger. Across **161,636 gate
observation ticks** covering every one of the eighteen paid children, the growth
branch was entered **zero times** and no child gained any structure.

The cohort also removes the single-lever reading of the earlier two-arm result.
On their own recorded intake, **nine of the eighteen children would still not
have reached the growth gate even if the entire oxidation sink were removed** —
their reserve inflow over their whole lives was smaller than the 0.4 they needed
above their birth escrow. Reducing the sink cannot be the whole answer.

Read the [independent review](hunter-juvenile-flow-independent-review-2026-09-13.md)
first: it sets the causal boundary these numbers live inside. Nothing here is a
counterfactual, a parameter recommendation, or authorization for a cohort,
retune, migration or live change.

## What was run

Nine new pinned replays, plus the two [already retained](hunter-juvenile-flow-diagnostic-2026-09-13.md)
ledgers reused rather than rerun.

| | |
| --- | --- |
| Executable | `captures/build-cache/juvenile-flow/release/examples/juvenile_flow` |
| Pinned SHA256 | `35943316416a947b6fedc42eaa973f787d2140f6fc53b95ca5360d6c52032977`, verified before reduction |
| Branch | `diagnostic/hunter-juvenile-flow-2026-09-13` at `0d867f1`, base `512ee52` |
| Inputs | the retained `captures/hunter-charge-candidate-two-hour-512ee52` arms, unchanged |
| New ledgers | `captures/juvenile-flow-cohort-2026-09-13/` (9 files, gitignored, each pinned by SHA256 in the committed reduction) |
| Reused ledgers | `captures/juvenile-flow-2026-09-13/` (seed-8 specialist, seed-6 facultative) |

No rebuild, no parameter change, no new seed, no care, no migration, no live
operation. Every arm ran the full 144000-tick horizon.

## Gates: 11 of 11 arms passed, nothing dropped

The [reducer](../../scripts/reduce-juvenile-flow-cohort.mjs) does not take each
replay's own "all four gates passed" on trust. It re-derives every gate against
the retained `opening.json` and `summary.json` the replay was meant to reproduce,
rejects a prefix run outright, and requires the descendant count to match the
arm's recorded `offspring`. A failing arm would be retained in the output with
its failure named; there were none.

| Arm | opening/closing identity | event records compared | reconciliation violations |
| --- | --- | ---: | ---: |
| 1 specialist_on | pass | 1135 | 0 |
| 1 facultative_on | pass | 1130 | 0 |
| 2 facultative_on | pass | 1221 | 0 |
| 5 specialist_on | pass | 948 | 0 |
| 5 facultative_on | pass | 942 | 0 |
| 6 facultative_on | pass | 757 | 0 |
| 7 specialist_on | pass | 935 | 0 |
| 7 facultative_on | pass | 984 | 0 |
| 8 specialist_on | pass | 997 | 0 |
| 8 facultative_on | pass | 925 | 0 |
| 12 facultative_on | pass | 1101 | 0 |

**Gate observations and reconciliation checks are different counts.** Over the
eighteen children: 161,636 gate observations against 161,654 reconciliation
checks. The 18 extra are one per child, at its birth boundary — a mid-replay
birth is registered during the commit that follows the physiology pass, so its
birth tick is probed but not gate-observed. A death then replaces that tick's
end-of-tick probe with one removal-site check, which is why the two counts match
for a member that was already present when the ledger opened. The reducer asserts
that identity per member rather than assuming it; it holds for all 29 members.

Worst reserve closure residual across the cohort: **4.26e-13**.

### Two independent measurement paths agree

The reduction cross-checks every child against the earlier census/transaction
reduction, which reads event streams and 200-tick boundaries rather than
mutation sites. **18 of 18 agree** on identity, birth tick, death cause,
censoring, lifespan and paid strike energy.

Two labelling facts came out of that comparison and are recorded rather than
smoothed over:

- The core emits its death event at `now + 1` while the ledger's `record_death`
  and end-of-tick probe use the pre-increment `now`, so a ledger `end_tick` sits
  exactly one below the event-stream death tick. No measured quantity depends on
  it — the convention-free check, that the ledger's gate observation count equals
  the event stream's birth-to-death span, holds exactly for all 18.
- Seed-12 `35:8` shows 17 attempts in the event stream against 5 paid strikes in
  the ledger. That is not a conflict: **12 of its attempts were `Unaffordable`**,
  meaning the child could not pay the 0.08 strike cost, so they never reached the
  charge site. Payable attempts equal paid strikes for all 18 children.

## Every child, by full ID

Reserve intake is digestion only for all eighteen — frugivory and grazing are
exactly zero everywhere, and scavenging is zero for every child (only one adult,
seed-6 `29:6`, ever scavenged, for 0.0297 reserve).

| Seed / arm | child | parent | born | end | ticks | reserve in | ox burn | max reserve | gate deficit | upkeep paid | strikes | digest ticks |
| --- | --- | --- | ---: | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| 1 S | 17:3 | 11:5 | 170401 | 179982 starved | 9582 | 0.5165 | 1.3165 | 0.7995 | 0.4005 | 2.489 | 8 | 217 |
| 1 S | 73:5 | 11:5 | 208802 | 214410 starved | 5609 | 0.0000 | 0.8000 | 0.7995 | 0.4005 | 1.560 | 4 | 0 |
| 1 S | 27:11 | 11:5 | 276004 | 283127 starved | 7124 | 0.0000 | 0.8000 | 0.7995 | 0.4005 | 1.880 | 0 | 0 |
| 1 F | 17:3 | 11:5 | 170401 | 179982 starved | 9582 | 0.5165 | 1.3165 | 0.7995 | 0.4005 | 2.489 | 8 | 217 |
| 1 F | 73:5 | 11:5 | 208802 | 214410 starved | 5609 | 0.0000 | 0.8000 | 0.7995 | 0.4005 | 1.560 | 4 | 0 |
| 1 F | 27:11 | 11:5 | 276004 | 283153 starved | 7150 | 0.0000 | 0.8000 | 0.7995 | 0.4005 | 1.880 | 0 | 0 |
| 2 F | 15:6 | 52:5 | 186151 | 197235 starved | 11085 | 0.4500 | 1.2583 | 0.7995 | 0.4005 | 2.497 | 5 | 216 |
| 2 F | 86:7 | 52:5 | 224552 | 233784 starved | 9233 | 0.4693 | 1.3035 | 0.9897 | 0.2103 | 2.527 | 7 | 167 |
| 2 F | 94:5 | 52:5 | 285107 | **censored alive** | 2893 | 0.4588 | 1.2196 | **1.0870** | **0.1130** | 0.787 | 2 | 175 |
| 5 S | 76:5 | 9:4 | 170401 | 176277 starved | 5877 | 0.1624 | 0.9624 | 0.7995 | 0.4005 | 1.600 | 8 | 80 |
| 5 F | 108:1 | 9:4 | 177536 | 186234 starved | 8699 | 0.0000 | 0.9827 | 0.7995 | 0.4005 | 2.294 | 0 | 0 |
| 6 F | 74:5 | 29:6 | 170739 | 179397 starved | 8659 | 0.3037 | 1.1037 | 1.0272 | 0.1728 | 2.396 | 2 | 121 |
| 7 S | 15:4 | 54:5 | 172294 | 176819 starved | 4526 | 0.0000 | 0.8000 | 0.7995 | 0.4005 | 1.240 | 8 | 0 |
| 7 F | 86:7 | 54:5 | 172294 | 184194 starved | 11901 | 0.7364 | 1.5893 | 0.7995 | 0.4005 | 3.168 | 7 | 350 |
| 8 S | 41:5 | 6:6 | 173805 | 196434 starved | 22630 | 2.7227 | 3.5227 | 0.8545 | 0.3455 | 5.867 | 30 | 1189 |
| 8 F | 53:6 | 6:6 | 171270 | 178000 starved | 6731 | 0.0000 | 0.8285 | 0.7995 | 0.4005 | 1.705 | 3 | 0 |
| 12 F | 11:9 | 38:4 | 183923 | 197113 starved | 13191 | 1.1514 | 2.1175 | 0.7995 | 0.4005 | 3.478 | 18 | 498 |
| 12 F | 35:8 | 38:4 | 222324 | 233878 starved | 11555 | 0.7204 | 1.5205 | 0.7995 | 0.4005 | 3.075 | 5 | 387 |

End ticks are the ledger's pre-increment labels; add one for the event stream's.
Seed 1's specialist and facultative arms produced children with the same IDs and
near-identical histories — they are separate worlds that happen to track closely,
and `27:11` does diverge by 26 ticks.

**Seventeen starved; one is censored alive** at the horizon after 2893 ticks.
Seven children never digested anything at all; three of those (`27:11` twice and
`108:1`) never made a single attempt.

## What the cohort establishes

**Growth never ran anywhere.** 0 of 18 children entered the branch, gained
structure, or reached adult structure. 0 of 18 ever had reserve above the 1.2
gate. The four growth cap-attribution counters remain unexercised by artifact
evidence, as the review noted.

**Reserve never rose above the endowment children were born with.** Fourteen of
the eighteen peaked at exactly 0.7995 — their 0.8 birth escrow minus a single
tick of oxidation. The cohort maximum, across every child and every tick, is
**1.0870**, and the closest any child came to the gate was a deficit of 0.1130.

**All reserve outflow was oxidation.** Cohort totals: 8.7101 in (digestion only),
23.0418 out, of which oxidation is 23.0418, growth 0.0000 and funding 0.0000.

**The above-reference classification separates adults from juveniles sharply.**
Cohort-wide, **3.55%** of juvenile reserve burn and 3.58% of juvenile oxidation
ticks were recorded above the world's configured 0.5 reference; **fourteen of the
eighteen children recorded 0.0%**. The eleven adult parents recorded 92.6%–100%.
This is a classification of recorded states on realized paths. It is **not** a
counterfactual: a different threshold changes the adult's stocks, its funding and
birth timing, prey interactions and the juvenile's own later encounters, so
nothing here says what a background-policy juvenile would have done, and nothing
here excludes adult or environment effects.

**Upkeep dominates the juvenile energy account.** Of 52.372 energy paid out
across the eighteen: upkeep 42.490 (81.1%), strikes 9.520 (18.2%), handling
0.362 (0.7%). Of 42.493 demanded upkeep: sensing 19.396 (45.6%), maintenance
16.164 (38.0%), movement 6.933 (16.3%). The clamped payment is not divided across
those terms anywhere in this reduction; demand and payment are reported side by
side. Payment fell short of demand on 17 ticks in total.

### The sink alone does not explain the cohort

Reserve has exactly one inflow (digestion) and, below the gate, exactly one
outflow (oxidation). So a bound follows directly from the recorded intake: with
the oxidation sink removed and nothing else changed, reserve would be monotone
non-decreasing at `0.8 + cumulative intake`, and would clear 1.2 only if intake
exceeded 0.4.

| | children |
| --- | ---: |
| Recorded intake above 0.4 — would clear the gate under that bound | **9** |
| Recorded intake at or below 0.4 — still short with zero sink | **9** |

That is a bound on each recorded intake sequence, not a counterfactual
trajectory: removing the sink would leave the child with more reserve and less
battery, changing when it strikes, whether it can afford to, and when it dies.
It is still decisive against one reading — **half the cohort fails on acquisition
regardless of what the sink does**, so no single sink-side lever accounts for
these eighteen outcomes.

## What is still unmeasured

- **Encounter and approach.** The ledger counts paid strikes, digestion ticks and
  the recorded `Unaffordable` refusals. It does not observe whether prey was ever
  within reach and not struck. For the nine acquisition-limited children — and
  especially the three that never attempted anything — nothing here distinguishes
  "no takeable encounter occurred" from "encounters occurred and intercept
  failed". That distinction is left open.
- **Any counterfactual.** No juvenile in this cohort ran under the world's
  configured threshold, so no comparison exists.
- **Dormant branches.** Reconciliation proves completeness only for stock
  movements a replay exercised.

## Smallest discriminating next step

The cohort splits the failure cleanly in half, and the two halves need different
evidence. The sink-side half is already measured. The acquisition-side half is
not, and it is the cheaper of the two to resolve.

**Extend the existing read-only ledger with one bounded per-member encounter
counter** and replay the same eleven pinned arms: per tick, whether any prey lay
within the member's capture-effector envelope while it was not already committed
to an attempt, and whether it struck. Nothing else changes — same branch, same
inputs, same four gates, one new counter, roughly nine minutes of compute.

That single addition answers the one question this cohort cannot: whether the
nine acquisition-limited children ever had a takeable shot. If they did, the
failure is intercept and the geometry is implicated; if they did not, the failure
is encounter and prey availability or sensing range is implicated. Either answer
points at a different lever, which is exactly why it should be measured before
any lever is chosen.

**No parameter change is proposed or authorized by this cohort.** An adult-scoped
charging threshold in particular is now weakly supported at best: the burn it
would remove is 3.55% of juvenile reserve burn on these paths, and half the
cohort is short on intake independently of it. No 24- or 72-hour hunter study
follows from a passing diagnostic.

## Verification

- Nine replays with the pinned executable, exit 0 each, all four gates passed;
  log at `captures/juvenile-flow-cohort-2026-09-13/run.log`.
- `node scripts/reduce-juvenile-flow-cohort.mjs …` — exit 0; 11 arms, 0 gate
  failures, 18 children, 0 cross-check disagreements, boundary identity holds for
  all 29 members. Committed reduction:
  [`assets/hunter-juvenile-flow-cohort-reduction-2026-09-13.json`](assets/hunter-juvenile-flow-cohort-reduction-2026-09-13.json).
- `node --test scripts/reduce-juvenile-flow-cohort.test.mjs` — 10 passed.
- The retained charging artifacts, the two earlier ledgers, the instrumentation
  branch and every original failure record are unchanged.
