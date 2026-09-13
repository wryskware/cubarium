---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Shared-prey cleanup: independent review of 512ee52

**No remaining blocker found for the unchanged twelve-seed charging rerun.**
This is a correctness/restart disposition, not a biological acceptance gate.
The [failed study](astra-hunter-charging-results-2026-09-13.md) and its original
invalid closing snapshot remain untouched. No new cohort or live operation was
performed for this review; no core change was needed.

Lore/Graft/canon and the existing paid-attempt contract were consulted. To avoid
mistaking formatter churn for semantics, both parent and committed versions of
the three changed files were normalized through the same formatter in memory
and diffed. The actual production changes are bounded:

- `HunterState::forget_target(id, tick)` compares the full slot/generation ID,
  clears matching targets, and ends unpaid Stalking/Windup in Perched at the
  completed removal boundary. Entered-from history records the interrupted phase.
- A paid Strike keeps its phase, timing, attack counter and episode; it receives
  no refund or retry. Same-boundary contenders settle from their existing copied
  attempt records, retaining TargetClaimed behavior. A later paid attempt without
  prey still settles through TargetLost normally.
- Capture and ordinary-death cleanup now use that helper. `remove_member` also
  uses it; repeated cleanup is inert. Natural prey death now clears an unpaid
  pursuit immediately instead of waiting for the next controller pass.
- Capture settlement clones the fixed profile to permit the mutable helper call;
  values and calculations are unchanged. There is no configuration/default,
  charging policy, cost, geometry, persistence shape, stream/key, or RNG-draw edit.
  Future trajectories may legitimately differ when the repaired branch is used;
  this is not a claim that active-hunter continuation is universally identical.

## Exact original failure, now valid

New independent tests in
[`astra_target_cleanup.rs`](../../crates/cubarium-core/tests/astra_target_cleanup.rs)
read the real retained seed-6 opening. They require the full state hash to remain
`2258426608805215987` through tick 170771. At tick 170772, the entire corrected
state equals the old invalid closing state's payload after **only** applying
`parent29:6.enter(Perched, 170772, 170772, 0)`. This is a full-state comparison,
not merely disappearance of an error message.

Child `74:5` still captures prey `63:5`, with paid attempt key 1, payment 0.08,
material `0.6035863309232207` and energy `1.0122994782514116`. Its capture phase
history, digestion, all other organism/field/ledger values, and inventory remain
the old values. Normal snapshot decoding now succeeds; reconstructed and
uninterrupted worlds match full state and both event streams for 300 more ticks.
The actual retained inputs were present and this replay ran, not the test's
documented absent-artifact skip.

A separate lost-target fixture forks that same paid-strike state and exports its
prey before settlement (a fixture, not an environmental-removal accounting test).
The target-free Strike round-trips through the ordinary decoder, then emits
exactly one TargetLost with the original paid key/payment, unchanged attempt
count, and Recovering entered from Strike. No extra payment or retry occurs.

The independent helper probe also checks an unrelated generation is completely
inert, both Stalking and Windup transition correctly, paid phase/timing/history
remain identical except the cleared handle, and a repeated invalidation does not
restart a pause. Submitted regressions cover natural-death Stalking cleanup,
same-tick contested capture, one carried body, TargetClaimed, checkpoint/restart,
generation reuse and gut transfer on hunter death.

## Executed checks

Using `CARGO_TARGET_DIR=captures/build-cache/ambient-review`:

- `cargo test -p cubarium-core --test hunter`: **28 passed**.
- `cargo test -p cubarium-core --test hunter_charging`: **17 passed**, including
  genuine pre-change version-3 600-tick continuation, ordinary-member isolation,
  paid conversion/funding and charging/gestation restart.
- `cargo test -p cubarium-core --test astra_target_cleanup`: **2 passed**;
  exact retained failure replay, lost-target restart and 300 subsequent ticks ran.
- Rebuilt `care_compare`; its entire seed-1/default-recipe/2400-tick stdout is
  byte-identical to the frozen `4d10351` executable. Both no-hunter arms, including
  the care arm and audit output, are preserved in this check.

All commands completed successfully. The worker's complete-core log was also
inspected, but this report's independent test counts are the bounded runs above.
No new scientific parameter family is warranted by the repair: freeze the fixed
code, rerun the original background/charging recipe on all twelve inputs in new
directories, retain the prior failure, and require the unchanged full-comparison
gate before drawing whole-cohort biological conclusions.

## Subsequent root integration and collection

Root froze production correction512ee52 in `/tmp/cubarium-charge-rerun-512ee52`.
Fresh hunter/charging/migration tests passed51/0/0; release build exited0.
Logs: `/tmp/cubarium-charge-rerun-{tests,build}.log`.
Executable SHA256 `65d42d78ecc3fc6fff4e9f5490d5e78df61f7f275ee8584e45221c3177b02828`.

Both original recipes are now collecting all twelve original inputs and all six
arms for144000 ticks, audit window200, with no care or parameter retuning:

- `captures/hunter-charge-background-two-hour-512ee52`, root handle30321,
  profile `reserve-targets-v1`.
- `captures/hunter-charge-candidate-two-hour-512ee52`, root handle47116,
  profile `reserve-targets-charge80-v1`.

Both manifests confirm build `0.1.0+512ee52`, the same recorded executable and
fixed horizon. Logs are `/tmp/cubarium-hunter-charge-{background,candidate}-two-hour-512ee52.log`.
These are running jobs, not completed results. Original3b06596 failures remain
untouched; the unchanged strict `charge80` comparison remains the complete-cohort
gate. Live meal buildd55d8af does not contain this experimental hunter correction
and still carries no hunter profile/member.
