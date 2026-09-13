---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent juvenile cohort reduction review

Reviewed `02f84ce623533210035d903aa8ace149576af002`, specifically
`scripts/reduce-juvenile-flow-cohort.mjs`, its ten tests, the cohort report and
committed reduction. Reducer source remained identical to that commit during
testing. All eleven retained ledger files match the SHA256 recorded in the
committed reduction. No replay, instrumentation, physiology, original artifact,
or live state was changed.

Disposition: correct the prose and reducer before using this as a verified
cohort aggregate. The observed absence of growth survives this review; several
intake claims and the claimed strength of reducer validation do not.

## Corrected recorded observations

Across 18 children, total reserve inflow is **8.710103516253374**:
digestion **8.208218126970925**, scavenging **0.5018853892824483** (5.76%),
and zero frugivory/grazing. Eight children scavenged:

| Seed / facultative arm child | Scavenged reserve | Total reserve intake |
| --- | ---: | ---: |
| 2 / 15:6 | 0.008299503834862047 | 0.45828817667941535 |
| 2 / 86:7 | 0.03425806560778589 | 0.5035446089662765 |
| 2 / 94:5 | 0.029054911700134634 | 0.4878694187419595 |
| 5 / 108:1 | 0.1827216171559611 | 0.1827216171559611 |
| 7 / 86:7 | 0.05291646736116771 | 0.789290251963437 |
| 8 / 53:6 | 0.028475757119234727 | 0.028475757119234727 |
| 12 / 11:9 | 0.16605462791010944 | 1.317489540771495 |
| 12 / 35:8 | 0.0001044385931927979 | 0.7205371766088587 |

The report's “reserve in” table substitutes digestion for total intake in these
eight rows. Seven children had zero digestion, but **only five had zero total
intake**. In particular, seed-5 `108:1` never struck or digested prey, yet did
acquire reserve by scavenging. Its zero-attempt history cannot be described as
zero acquisition. The reducer's `cohortTotals` computes the incorrectly named
`children_with_zero_reserve_intake` from digestion alone (lines 252–263).

Seven adult parents also scavenged (facultative seeds 1, 2, 5, 6, 7, 8, 12), not
only seed-6 `29:6`, totaling 6.061456848380978 reserve. The children's scavenging
also supplied 0.3345902595216321 battery energy. This is not an unexplained stock
residual; the source-specific ledger records account for it.

Four children exceeded their 0.8 birth reserve at the observed growth gate:
seed-2 facultative `86:7` **0.9897406062818958**, `94:5`
**1.0870443357568322**, seed-6 facultative `74:5`
**1.0271898434754285**, and seed-8 specialist `41:5`
**0.8544761980304785**. Fourteen instead peaked at approximately 0.7995.
Thus “reserve never rose above the endowment” is false; “none cleared 1.2”
remains supported. These are maxima at the ledger's gate-observation site,
not a claim to observe every intra-tick stock assignment.

Unchanged findings: 161,636 child gate observations, 161,654 reconciliation
checks, zero growth steps/structure gain/adult ticks, seventeen starvation
endings and one right-censored child. All recorded child reserve outflow is
oxidation. Independently checking all **29** members finds valid boundary
identities, zero member violations, finite nonnegative endpoint stocks and
finite nonnegative reported maximum residuals below the diagnostic's 1e-9
tolerance. Largest child lifetime reserve closure residual remains
4.263256414560601e-13. This distinguishes sound retained values from missing
validation in the reducer.

## Decision-worthy reducer gaps

1. **Acceptance does not require a complete unique cohort.** `reduce` enumerates
   arbitrary JSON files in supplied directories (lines 302–306), without the
   expected eleven arm keys or exact unique child coverage. Removing an arm
   yields ten accepted arms; duplicating one yields twelve. `crossCheck` searches
   only supplied children, so a missing ledger child is not detected in reverse.
   Derive the expected birth-producing arm/child set from the retained cohort
   and census identities; require exact unique coverage rather than only each
   supplied arm's offspring count.

2. **Displayed failures need not fail the command.** The CLI checks only
   `arms_with_gate_failures` (line 417). A deliberate census disagreement and
   a false child boundary both still exit 0. Moreover, the top-level boundary
   verdict uses `children.every(...)` (line 373): a broken founder boundary
   leaves it true despite the report claiming all 29 members. Include every
   member and propagate coverage, cross-check, and boundary failures into the
   single completion verdict/exit status while retaining diagnostics.

3. **The four gates are not independently re-derived as advertised.**
   `checkGates` (lines 34–69) compares opening/closing labels with retained JSON,
   trusts observer-neutrality booleans, and trusts aggregate violation counts.
   It neither replays event comparisons nor reconstructs per-tick reconciliation.
   It even ignores contradictory member violation counts, excessive/negative/
   nonfinite maximum residuals, and invalid endpoint stocks. Validate the
   available member evidence with the fixed diagnostic tolerance and derive
   totals from mutation-site components; accurately label the original replay's
   asserted event/byte neutrality versus the reducer's artifact checks. This
   does not call for another replay or a new transaction API.

## Preserved probes and verification

`node scripts/astra-juvenile-cohort-review.test.mjs` against `02f84ce`:
**2 pass, 12 fail**, intentionally preserving desired-behavior regressions.
The two controls verify the actual retained member values and reproduce complete
11-arm/18-child CLI acceptance. The twelve red cases cover the incorrect intake
count, six member-validation mutations, and five CLI acceptance defects above.
CLI probes copy small JSON fixtures to unique temporary directories and execute
only Node, never the diagnostic binary. Their executable SHA fixture is explicitly
synthetic. Child-process execution needed sandbox escalation here; the first
sandboxed attempt's EPERM is not counted as a reducer finding.

The original ten reducer tests pass. Passing those tests does not close the
twelve independent failures. Tests read the retained evidence at runtime and
verify its recorded hashes; no large raw ledgers are committed again.

## Causal limit and next-step scope

The **9 above / 9 at-or-below 0.4** split survives when computed from total
intake, including scavenging. It supports a fixed-recorded-sequence arithmetic
bound only: replacing oxidation with zero while freezing all other recorded
flows would not lift nine children above 1.2. It does not show that half would
fail “regardless of what the sink does.” A changed sink changes battery,
survival duration, later intake opportunities and potentially parent histories;
those trajectories were not observed here. The censored child's total is also
horizon-bounded, not a completed lifetime.

A future encounter diagnostic may distinguish absent opportunities from unused
opportunities, but cannot infer a geometry defect merely from reachable but
unstruck prey: payment, controller mode, handling and commitment remain possible
causes. Preserve scavenging as an acquisition source. Correct the reduction and
description first; this review does not authorize a new replay or tuning step.
