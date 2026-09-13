---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Quiet comparison observer: independent boundary review

Review of the completed initial observer package `6d4ac7b`, after reading canon,
Lore/Graft context and the entire [ordinary quiet proposal](astra-ordinary-quiet-experiment-proposal-2026-09-13.md).
Scope is behavioral classification/reconciliation, not the parent's separate
manifest, audit and reducer-provenance review. The correction worker is active;
this is **not** a finding against its future corrected package.

## Reproducible failures

The separately owned example
`crates/cubarium/examples/astra_quiet_observer.rs` includes the observer by path
and supplies six independent fixtures. It changes no worker file or core behavior.
Its one-parent synthetic setup performs a genuine core escrow funding and child
insertion at B=3. Fixture-only stock/age changes isolate boundaries; they are not
the copied-world ecology recipe or balance evidence.

```text
CARGO_TARGET_DIR=captures/build-cache/astra-quiet-harness-review \
  cargo test -p cubarium --example astra_quiet_observer astra_ -- --nocapture
```

Result: **0 passed, 6 failed, 7 filtered out**, exit 101. Before/after source
checks show the tested `quiet_compare/bouts.rs` is identical to commit `6d4ac7b`,
SHA-256 `371c47cd4db6a6da08ed20538faf3d7fa4a78ce550363537040cd61db7fece89`.
The seven filtered tests are the included worker module's own tests, not seven
independent successes. All six checked-in assertions express desired behavior
and deliberately remain red until the worker correction.

### 1. Successful release into ordinary Resting loses its terminal reason

`Observer::after` labels a recovery bout's terminal event only in the active-mode
branch. If the ordinary controller legitimately chooses Resting after expiry,
the class-change branch instead closes recovery as `Reclassified(Satiated)`.
The fixture completes all forty held intervals and receives one real End, but
the bout is not `Released`. The reducer separately requires released bout count
to equal End count, so a valid world can be refused. Early abort into ordinary
Resting follows the same class-change branch and needs equivalent handling.

Preserve the recovery terminal reason from the matching End/Abort **before**
opening any subsequent natural-rest bout; continuing to rest does not cancel a
completed recovery release.

### 2. Death on the fortieth held interval is valid, not an overlong abort

An age death on completed B+40 follows the final held decision at B+39. Core
correctly emits `Abort(ParentGone, completed_ticks=40)`. The observer rejects all
aborts with `completed_ticks >= 40`, falsely marking this valid trajectory
unreconciled. Its post-state-only classification also records a 39-tick recovery
bout because the parent is no longer in the arena, despite the record confirming
forty executed intervals. The fixture exposes both facts: class recovery ticks
39 versus core completed-held ticks 40. The reducer has the same unconditional
abort-duration rejection and must use the corrected boundary contract too.

Distinguish pre-decision affordability aborts (the current interval is ordinary)
from post-decision ParentGone aborts (the current held decision executed).
Preserve the completed dead-parent interval in the recovery duration. If general
population organism-time remains a post-step census measure, name that denominator
explicitly rather than silently conflating it with executed decision intervals.
No unavailable dead-parent post-movement pose or intake amount should be invented.

### 3–6. Quiet event reconciliation is not one-to-one with actual state changes

Four independent malformed-stream fixtures are currently accepted with
`reconciled=true`:

- Duplicate a genuine Begin: the same paid birth increments admissions twice.
- Remove Begin while retaining the real newly inserted pause and actual birth.
- Append an Unaffordable Refuse for absent full IDs (slots 999/998, generations
  7/8), with no child insertion or matching life event.
- Remove the genuine End on release: the bout silently becomes Woke/Reclassified.

`Begin` currently searches for any matching LifeEvent but does not enforce
cardinality against the new actual pause. Refuse merely increments a reason
counter. End/Abort do not validate their full originating identity and expected
pre/post transition. This can change opportunity denominators or terminal counts
without making the harness's reconciliation gate fail.

For this fixed policy, compare the full pre/post pause sets and actual committed
birth identities with the event multiset at the exact boundary: every new pause
needs exactly one Begin; a rejected opportunity still needs its real birth;
every disappearing pause needs its matching release/death/abort, including the
child and start identity. Reject duplicates and missing records, not merely
impossible aggregate totals. Keep violations bounded and retain failure; no
transaction API expansion or changes to core reproduction payments are needed.

## Measurement limits to keep explicit

The proposal also asks for active-to-recovery versus natural active-to-rest
entries. The initial observer records classes and bout durations, but does not
emit prior-mode/active-entry evidence. Not every admitted parent is necessarily
active at admission; do not equate admission count with interrupted-foraging
count. Either expose this small classification distinction or label it unavailable
before interpreting the behavioral screen. The one-second/9-of-12 screen must
remain about genuine non-newborn recovery, separate from biological viability.

The initial module's same-face coordinate displacement is not a guaranteed
transported path length at a reflected rim; a body can reverse within the same
face. The parent already permits a minimal read-only movement accessor in the
correction task. It must expose actual core segments without changing simulation,
RNG, stocks or schema. Any unmeasured dead-parent segment remains unavailable.

The parent separately owns the discovered `verify_resume` comparison of two
decoded worlds (not uninterrupted versus resumed), the ineffective loader fixture,
and strict reducer/provenance gates; those are not duplicated here.

## Disposition

Do not launch the 48-arm biological screen or certify completed measurement from
the initial observer. The six red fixtures are a correction handoff, not a claim
that the future worker package fails them. Retest the exact final commit, then
distinguish passing mechanism/observer gates from actual quiet opportunity and
the unchanged no-care survival/diversity requirements. No core, art, live state,
care recipe or canonical decision changed in this review.
