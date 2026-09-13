---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent review — corrected juvenile cohort reduction

## Verdict

Reviewed correction `c045bf6` against the original independent regression
`17a998c`, the retained eleven-arm source tree, and a fresh reducer invocation.
The corrected reducer is suitable to report the **realized-path** eleven-arm,
eighteen-child reduction. It reads both retained snapshots for every arm rather
than inferring identity from matching JSON labels, preserves cohort completeness,
and leaves the causal boundary explicit.

This is evidence review only. It authorizes no replay, parameter change, live
operation, biological conclusion, or threshold counterfactual.

## Fresh checks

- The real reducer completed against the retained cohort: 11 valid arms, 18
  children, 29 members, 0 invalid arms, 0 boundary failures, and 0 census
  cross-check disagreements.
- All 22 retained snapshots were read as schema 12 with byte-derived SHA256 and
  state hash data. The reducer compares them to the arm JSON and report before
  including an arm in statistics.
- Corrected realized-path counts agree with the source ledgers: eight children
  scavenged; five had zero **total** reserve intake while seven had zero
  digestion; four rose above birth escrow; none cleared the 1.2 growth gate.
- `node --test scripts/reduce-juvenile-flow-cohort.test.mjs` continues to cover
  the reducer’s own malformed-record, frozen-tolerance, intake, membership,
  cross-check, and completion behavior.

## Original independent regression repaired, not weakened

The original CLI fixture copied only JSON. That had correctly become invalid
when `c045bf6` required `post-initialization.cubw` and `closing.cubw`: its
control never exercised byte verification and all arms failed
`snapshot_unreadable`.

The independent fixture now copies each arm's two original retained snapshots
into its unique temporary arm directory. It does not execute, overwrite, or
modify those source files. The substitute executable remains a SHA fixture.
The control now accepts 11 arms, 18 children, and 29 members only after each
temporary arm reaches schema-12 snapshot verification. All five original
negative mutations remain and name their intended refusal: missing/duplicate
cohort membership, census birth-tick disagreement, and child/founder boundary
identity failures. Result: **14 passed, 0 failed**.

## Evidence boundary

The corrected reducer/report draw the important distinctions correctly:

- Snapshot identity is independently recomputed from retained bytes; observer
  neutrality and reconciliation remain replay-reported claims, validated for
  record shape and the reducer's frozen `1e-9` threshold rather than rerun.
- The above-reference accounting classifies recorded states, not outcomes under
  a different charging threshold.
- The `escrow + recorded intake` result is a fixed-intake bookkeeping ceiling,
  not a causal allocation of failure. A changed sink can change energy,
  lifespan, movement, affordability, encounters, parent funding, and birth
  timing.
- The growth branch and cap counters were dormant; reconciliation establishes
  only executed stock movements. Encounter/approach opportunity remains
  unmeasured.

## Release-note correction

The committed native report's verification section still says this independent
test is “13 passed, 1 failed” and describes the JSON-only fixture gap. That was
accurate before this fixture repair but is now stale. Update it to **14 passed,
0 failed**, while retaining the substantive statement that missing snapshots
must fail rather than be treated as reduced evidence.
