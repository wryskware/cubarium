---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent review of matched hunter recipe reduction

Reviewed `35861cc`: `scripts/compare-hunter-recipes.mjs`, its tests and the explicit
expected-schema parameter of `prepare-hunter-worlds.mjs`. Lore and actual saved
opening/summary/event schemas were consulted before making artifact claims.
No trial, source worker or live process was modified or restarted.

Three narrow consistency checks should be corrected before using the reducer's
`artifact_checks_passed` flag as the report gate:

1. **Strict numerical boundary:** `verifyArm` accepts `magnitude <= limit`, while
   the harness's `audit::Peak::observe` records a failure at `abs(x) >= limit`.
   A summary claiming equality and no crossing is contradictory and currently
   accepted. Require strict `<`, preserving the harness's existing tolerance.
2. **Joint-stock intersection:** the upper bounds are correct, but the lower
   bound is absent. With ten mature member-ticks, reserve-open ten, energy-open
   ten and both-open zero passes. Require
   `both >= max(0, reserve + energy - mature)` in addition to current bounds.
   This checks the reducer's own reported denominators without reconstructing
   any mutation-site funding event.
3. **Missing coordinates become a biological category:** `reduceEvents` checks
   the presence of `measure` and `geometry`, but not the numeric coordinates used
   by its comparison. `measure.body = {}` becomes `undefined < x`, hence false,
   and increments `far_out_of_reach`. Require finite numbers for both x values;
   test absent, null and nonnumeric coordinates on both sides. This is validation
   of inputs to one summary comparison, not a new contact-geometry validator.

The accompanying
[`astra-hunter-reducer-probes-2026-09-13.mjs`](astra-hunter-reducer-probes-2026-09-13.mjs)
contains three standalone bug-demonstration tests. They passed against `35861cc`
and intentionally should stop passing when corrected. The first two gaps were
also reproduced on in-memory clones of actual seed-1 specialist-on summary data;
no capture file was changed. None of these findings shows that an actual saved
arm violated its numerical or observer gate.

## Checks that hold within this reducer's scope

- Mature ratios use age-and-adult-size-ready **member-ticks**, with null for no
  denominator. Occupancy's denominator is world ticks. Paid energy per capture
  is null when captures are zero. The code does not conflate these denominators.
- Hunter offspring are counted from hunter events and reconciled with exact
  reproduction outcome counts. `observed_maturity` is emitted by the harness only
  while iterating hunter membership; depth > 0 there means a hunter descendant.
  The unrelated maximum live ancestry depth over prey and hunters is not used as
  evidence of hunter replacement. Per-form losses use prey-only whole-recovery
  channels, skipping the total channel.
- Opening pairing compares both configs, imports, headings and targets, then
  permits exactly .35/.65 to .80/.90 in stored profiles. Untouched summaries must
  match fully; budget controls exclude only the two full-state hashes, because
  their stored profile legitimately differs. Matching recipe metadata is not
  semantic decoding of those fields from the binary payload, and the tool says so.
- The full report requires global summaries, all twelve unique prescribed seeds,
  six arms each, matching per-seed/per-arm summaries, observer coverage and local
  capture/selection counts. Missing global summary is not partial success.
- Snapshot inspection still defaults to schema 9 for preparation. The reducer
  explicitly requests frozen schema 11 and verifies envelope length, CRC, SHA
  and full payload hash. It neither relabels files nor claims semantic migration;
  the relabeled unit fixture is expressly an envelope-only test.

## Bounded verification

Direct foreground runs under Node v26.8.2:

- `node scripts/compare-hunter-recipes.test.mjs`: **7 passed**, exit 0.
- `node scripts/prepare-hunter-worlds.test.mjs`: **6 passed**, exit 0.
- The three independent bug demonstrations: **3 passed**, exit 0.

An additional read-only check covered seeds 1 and 2 in both running directories:
**24 completed arms**, including arm coverage, event capture/offspring counts,
both saved snapshot CRC/SHA/full-state hashes and paired opening profiles. Those
checks passed. This is intentionally narrower than root's earlier 96-arm check;
it is not a new complete aggregate.

The full command refused with exit 1 because
`hunter-reserve-baseline-two-hour-b547ad0/summary.json` was absent. That correctly
preserves the in-progress distinction. No full-cohort results or biological
acceptance are inferred here. The reducer remains a convenience for retained
evidence, not a replacement for the core audits, transaction reconciliation or
contact reconstruction already performed by the experiment.

## Closure against f0ad8ef

All three findings are **resolved** by `f0ad8ef`. Reran the unchanged historical
bug demonstrations against the corrected source: they exit 1 with all three
former false-accept assertions failing for the intended validation reasons.
These are deliberately preserved bug demonstrations, not expected passing
regression tests on the corrected version. A separate inverse-assertion check
confirmed all three malformed inputs now throw, while the unchanged actual
seed-1 specialist-on summary still passes.

Foreground regression suites now report **9 reducer tests and 6 preparation
tests passed**, both exit 0. The new tests cover strict equality, the missing
intersection lower bound, and absent/null/nonnumeric/nonfinite axial coordinates
on both sides. No existing test was removed. This closes the bounded reducer
review; it does not advance the still-pending full-cohort completion or biological
acceptance gates. Production and trial files remain root-owned and untouched by
this review.
