---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Actual-intake meal continuity: independent boundary review

Reviewed the committed `8d489f7` meal module, presenter integration and 13 tests,
using Lore/Graft then actual source. The implementation is presentation-only:
`fed` selects observed intake bouts, not an inferred care-source attribution,
meal quantity, satiety, attraction or new biological rest. Mode-only Feeding
without observed intake retains its old behavior. Fable's selected creature
silhouettes and the ecology remain unchanged.

## Confirmed capture-boundary defect and scoped closure

The new ordinary meal was drawn with its own bout-time feed phase. When that
organism became retained prey, `stamp_creature` received **no meal or body
memory**, so the prey switched back to its old shared phase at the start of the
capture interpolation tick. Clearing the living meal map was correct; discarding
the outgoing displayed pose before the one-tick handoff was not.

The preserved [public-API fixture](assets/astra-meal-capture-boundary-2026-09-13.rs)
has two controls using synthetic published views: a remote hunter prevents its
own pixels hiding the prey difference. It does **not** claim that this deliberately
remote synthetic capture is ecologically admissible. Against the pre-fix built
host, the shared-clock control was exact, but the enabled meal changed **7 prey
pixels at one identical presentation instant**, with maximum linear-channel
difference **0.86243135**. Thus this was a new observable meal handoff regression,
not a preference for a different interpolation formula.

Root explicitly authorized the narrow source correction after confirmation.
`ArtPresenter` now copies outgoing body/meal memory for previously observed hunter
targets that disappear, **before** pruning living bodies and meals. After hunter
observation, only actually retained prey may keep this copy. Each entry is keyed
by full organism ID and tagged with this capture tick; the next ordinary tick or
rewind clears it. Empty/dropped hunter membership discards provisional copies.
Repeated observations of the same boundary preserve it. A copy is never
fabricated on first observation/restart.

Retained drawing uses the previous completed tick's exact `weight_at(1)` and
onset, plus its outgoing body cross-fade. The bout clock may continue through the
existing one-tick transport; no new intake, fade target, satiation or persisted
state is inferred for removed prey. Keeping the body fade matters when the meal
weight is still partial. The current settlement-position/chord approximation,
capture timing, hunter rig and prey deletion time are unchanged.

The fix is confined to `art_present.rs` and the new
`crates/cubarium/tests/meal_capture_boundary.rs`. `meal_present.rs` needed no change.
The review switch disables the meal modifier; both halves now share correct
retained-body cross-fade handling. Accordingly it should not be described as
bit-identical to an older renderer's missing body-fade history at every capture.
No-hunter views cannot create these target copies and keep their existing output.

## Verified scope

- Existing meal target: **13 passed**. Includes no-intake identity, onset/settle/
  return, first-seen fed, full-ID reuse, skipped membership, rewind, duplicate
  observations, one actual surface-helper seam trajectory, and established
  meal→gestation continuity. Its named 30/60/120 test compares common instants and
  redraw behavior, not independent browser playback cadences.
- New handoff target: **4 passed**. Established-meal and disabled-clock controls
  match at the capture boundary; a partial meal preserves its underlying body
  cross-fade; repeated same-tick observation holds the picture and the next tick
  drops the prey; rewind produces the same meal/image as a fresh presenter.
- Existing hunter target: **29 passed, 3 ignored**. Includes real core capture
  and miss fixtures, settlement-carried prey, seams/rim/vertex cases, observer
  draining, scale and rewind behavior. Ignored visual/performance captures were
  not rerun for this narrow fix.
- Preserved two-test independent probe now passes against the corrected host.

Targeted command (root's existing build cache; no large `/tmp` build):

```sh
CARGO_TARGET_DIR=captures/build-cache/fable-meals cargo test --offline -p cubarium --test meal_capture_boundary --test meal_present --test hunter_present
```

Memory remains bounded: the living meal map is pruned every new observation;
outgoing copies are at most the previous hunter-target count before observer
pruning, then at most actual retained prey for this tick. There is no accumulating
dead-ID archive, image history, randomness, world mutation or schema addition.
Normal host usage observes every completed tick; the public module deliberately
does not reconstruct unobserved intake history after arbitrary skipped ticks.

## Disposition and limits

The concrete new handoff regression is closed by the focused tests. No further
meal-specific correctness blocker was found in this bounded source review.
Root's clean full-workspace verification and final deployment build remain its
integration gates; the 46 targeted passing tests above do not substitute for them.

This review does not approve subjective meal readability or claim biological
benefit. Final candidate captures are `captures/meal-reviewed-8d489f7/`, not the
earlier pre-nibble-hold `meal-onset-*` material. Root owns the matched seed1/seed8/
no-input playback review. No live process, player or shim was touched here.
Genuine post-meal quiet physiology remains separate work.
