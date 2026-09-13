---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Paid charging: independent implementation review

Reviewed core `b4dfd1b` and harness/reducer `83e562e` against the
[charging proposal](astra-hunter-paid-charging-proposal-2026-09-13.md), using
AGENTS/Graft/Lore context and actual source. The paid physiology family is faithful.
Four evidence/diagnostic gaps were identified and closed in this scoped follow-up,
with root's explicit authorization; **the conversion or experimental policy was
not changed**. Clean integration/build verification remains root-owned. This is
readiness for a matched experiment, not biological acceptance or live permission.

Checkpoint ownership: native Opus integrated the authorized core/reducer corrections
as **3b06596**, alongside its own zero-rate regression. Astra committed only the two
independent regression files and this review as **c2fd19e**. Scoped diff/status checks
confirmed that no correction was lost; unrelated Fable art is not part of either
of those commits. Use root's clean charging freeze rather than a moving checkout.

## Core and persistence findings

- Default constructor/profile version remains 3; new validation accepts exactly
  3 and 4. Version 3 resolves the configured threshold unchanged. Version 4 alone
  resolves fixed 0.80. Serialized profile shape and all other fields are unchanged.
- World physiology selects the policy by authoritative membership, with no
  special age, phase, reproductive or reserve-floor condition. The existing
  reserve decrement, local nutrient return, release, efficiency, headroom cap and
  heat arithmetic remain the same block, before existing growth/reproduction.
  Costs, geometry, gates and offspring recipe were not changed by these commits.
- Charging diagnostics live on `World`, not `WorldState`, and the step never
  reads them to decide biology. Access returns a copy; reconstruction resets them.
  They count real activation-band transactions on the realized candidate path,
  **not** a counterfactual net resource difference from the divergent reference.
  The original repeat-run diagnostic test proves determinism, not independently
  that instrumentation was disabled; source isolation and the reset/replay test
  supply the narrower non-interference evidence.
- Genuine pre-change v3 active fixture continues 600 ticks with exactly equal
  encoded payload. Independent SHA256 checks matched all three committed fixture
  hashes and the stated old executable. The provenance paragraph claiming that
  `SCHEMA_VERSION==12` and `PROFILE_VERSION==3` alone rule out the changed build
  is too strong: both remain true now. The frozen commit/tree/binary provenance,
  not those assertions, is the relevant pre-change evidence.

Independent old-reader execution used only fresh `/tmp` state directories and
copies of seed-1 specialist-on smoke snapshots:

```text
old binary: /tmp/cubarium-pre-charge-fDU26F/target/release/cubarium
SHA256: b32db44484077bcccced02929c694c71c2737f55b02189fc23b30406a34ad309
run --state <isolated-copy> --require-resume --sink none --speed 0 --seconds 0.05

v4: /tmp/astra-charge-old-reader-v4-1Ph41k
    Invalid("hunter profile version 4 is not 3"); exit 1; no new world created
v3: /tmp/astra-charge-old-reader-v3-zb4ewY
    resumed tick 144000, completed tick 144001; exit 0
```

This verifies same-shape semantic rejection, not merely newer-schema rejection.
Original snapshots, smoke artifacts and the old executable were untouched.

## Concrete findings and closures

1. **Zero-rate false transaction.** A valid v4 world with oxidation_rate=0
   incremented `extra_transactions` to 1 with zero burn/gain/heat. The reducer
   then rejected this legitimate output as a transaction without a burn. Saved
   `astra_charging_regressions.rs` reproduced it (one fail, one pass before fix).
   The only core correction is `above_reference && burned > 0.0` on diagnostic
   accumulation; physiology itself still executes the identical arithmetic.

2. **Unpaid finite diagnostic totals accepted.** `verifyOxidation` accepted one
   transaction burning 0.0005 material and gaining 100 energy. It also lacked the
   efficiency and per-transaction rate bounds. New checks reconcile release with
   gain+heat, cap gain by conversion efficiency, and cap burn by transactions ×
   oxidation_rate × the frozen 0.05-second core step. Count is bounded by planned
   ticks × configured population cap. Nonfinite derived bounds fail. A conservative
   nonnegative-sum floating-point envelope allows accumulation rounding; it is not
   a tuned biological result or an independent re-execution of the transactions.

3. **Threshold labels could disagree with actual config.** Matching opening
   labels of 0.50 passed even when both actual configs said 0.60; a summary saying
   0.40 also passed against a 0.50 opening. The new checks link config, opening,
   manifest and summary explicitly. `verifyOxidation` now requires the opening,
   not just a duplicated label. Three independent reducer probes failed before
   correction and now pass; they remain in their own test file.

4. **Budget-control identity was unchecked.** Charging changes no member because
   that arm has none; its full-state/snapshot hashes differ only because a profile
   is stored. The new control projection omits only those two hash fields and the
   reported hypothetical member threshold, then requires complete summary equality,
   including the ecology-projection hash. It also requires zero member ticks,
   closing hunters and extra charging. Untouched retains full equality, while
   attack-disabled living hunters correctly remain free to diverge. Adversarial
   prey integral, ecology hash, member-count and charging changes are rejected.

## Actual validation and its limits

- Initial submitted tests: core charging **16 passed**, reducer **11 passed**.
- After correction: independent core **2 passed**, submitted charging **16 passed**;
  complete `cargo test -p cubarium-core` exited 0. Native Opus subsequently added
  its own zero-rate test to its original file and committed it in 3b06596; that
  edit is not mine and is excluded from Astra's three-file checkpoint.
- Independent reducer **6 passed**, existing reducer **11 passed**, and focused
  core regression clippy with `-D warnings` passed. The independent funding test
  uses the unchanged 1200-second age gate, actual 0.80/0.75 stock gates, fixed
  reserve-target background and real upkeep: v4 charges across the gate, emits
  the actual Funded record, debits exactly 1.6 R / 1.0 E and stores the unchanged
  0.8 S / 0.8 R / 0.6 E escrow with 0.4 build heat. The paired v3 does not fund.
  Hand-set initial fixture age/stocks are not survival or acquisition evidence.
- Read-only validation of actual `charge80-smoke-83e562e-reserve-targets-v1`
  versus `charge80-smoke-83e562e-charge80` passed all **72 paired arms**, each
  2000 ticks: opening/manifest thresholds, conversion/rate bounds and no-member
  control identity. This did not bypass or invoke the full-cohort reducer's
  minimum 144000-tick requirement. No full charging cohort was launched here.
- Re-ran the old full `compare()` on the immutable b547ad0 baseline/reserve-target
  artifacts: all twelve seeds still reduce with artifact checks passing. Its
  separate schema-11/two-reserve-field contract remains intact.
- Host capability source/tests were read, not rerun against Fable's dirty art.
  They compare unchanged effectors/scale and a supplied frame across versions.
  The listed “role damage” is a no-op (it assigns the existing Lanternjaw role),
  and a nonblack world frame alone does not prove a visible hunter. Those are
  test-description limits, not evidence that the version-only core change alters
  the selected silhouette. Root owns clean host/integration validation.

The remaining gate is clean frozen integration of these corrections and the
authorized matched twelve-seed/six-arm study, with all numerical and reproduction
audits active. More battery, one paid synthetic escrow, or a successful short
smoke cannot establish persistence, descendants, harmless prey effects or local
recovery. Preserve the proposed coupled reserve/energy deficit as a possible
negative experimental outcome.
