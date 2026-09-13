---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Viewer amount selector: bounded independent review

Reviewed `431705b` and root browser evidence `3194baf`. The current inline viewer
and `scripts/viewer-care-dose.test.mjs` match the reviewed commit. Used Lore to
retrieve the dose handoff, read actual source and current canon, and inspected
the two provided screenshots. No production changes, backend approval, live or
fixture process operations, shim access, or world mutations were performed.

**Disposition:** no concrete blocker found in this viewer amount-selector slice.
Backend quantity, admission, journal, snapshot and replay guarantees remain
outside this result while the native implementation is in progress.

## Behavior checked against actual inline JavaScript

- Capability is admitted only for version exactly 1, integer bounds contained
  within 250..=2000 and containing standard 1000, and default exactly 1000.
  These inequalities also reject reversed bounds. Missing, null, malformed and
  unknown-version capabilities fall back to disabled standard-only selection;
  the payload then omits `dose_permille`, preserving the legacy request shape.
- Narrower valid bounds disable unavailable preset options and reset an invalid
  selection to standard. Submission separately rechecks the three offered preset
  values and negotiated bounds; editing a disabled select cannot raise a legacy
  request's amount. This UI does not claim to expose every integer the typed API
  may support.
- `send` snapshots the selected numeric dose and copies target coordinates before
  serializing the request. The serialized POST body and callback's action label
  cannot be rewritten by later selection/target changes. One action produces one
  request and one chosen total dose; the UI adds no queue of repeated actions.
- Local queued/accepted/refused/lost-connection rows use the captured action label.
  A 202 without sequence is still committing, not applied. Status receipt rows
  use the server's own `dose_permille` and quantities, not the current selector.
  A legacy receipt with omitted dose is labelled standard. Connection failure is
  explicitly unknown; refusal does not claim food, rain or cleanup occurred.
- Disabling care clears negotiated capability and prevents further submission.
  Amount controls remain outside the image canvases; the selector adds no world
  frame overlay, HUD, ambient multiplier or simulation mutation.

## Exact tests and evidence scope

Ran `node scripts/viewer-care-dose.test.mjs`: **10 passed, 0 failed**, exit 0.
The harness extracts the actual inline care script, not a handwritten replacement
of the functions. It checks markup/defaults, legacy omission, all action payloads,
in-flight selection/target changes, receipt labels, malformed/narrower capabilities,
invalid selections, disabled care, refusal and connection loss.

The mock DOM/transport is intentionally limited: event listeners, focus, native
select interaction, CSS layout, browser accessibility tree and real HTTP admission
are not exercised. Its in-flight test mutates selection and target before the
mock response callbacks settle, establishing snapshot behavior; it is not a test
of a durable backend accepting a delayed real request. These are ten tests, not
the single aggregate file count some `node --test` invocations display.

Independently viewed `/tmp/cubarium-care-dose-ui-desktop.png` and
`/tmp/cubarium-care-dose-ui-mobile.png`. The screenshots visibly identify a fixture
with no ecosystem attached. Desktop shows the chosen Generous amount and an
explicit refusal saying no world changed. At narrow width, Amount/select and
action buttons wrap within the care panel; help text wraps below them. Root's
record reports no horizontal document overflow at 1400×1000 and 390×844 and the
exact desktop wire payload; I did not reopen the stopped fixture or independently
remeasure DOM bounds or submit a browser request.

## Accessibility and limits

The new native `select` has an explicit associated `label`, an initially selected
Standard option, disabled-state semantics, and `aria-describedby` pointing to the
capability explanation. The optional native details panel remains closed by
default. These are useful native control affordances, not a complete accessibility
audit or a screen-reader test.

Existing broader care-panel limitations remain: target selection is pointer-only
on the canvas, status/receipt containers have no live-announcement role, and long
receipt rows are visually ellipsized rather than expanded on narrow screens.
No keyboard-only targeting, screen-reader receipt notification, contrast conformance
or exhaustive touch-target assessment is established here. They are not regressions
introduced by the amount selector and should be tracked separately from backend
dose correctness.

The next integration gate is actual typed-host acceptance/refusal and receipt
verification on copied/synthetic worlds, including dose-bearing durability and
legacy behavior. This review does not authorize live rollout or imply those gates
have passed.
