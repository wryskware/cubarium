---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Care browser review and rollout record

## Initial copied-world API check

Root launched an isolated test runner at `http://127.0.0.1:7398/` from a copy of
the genuine schema-7 checkpoint at tick 67,984. State directory:
`/tmp/cubarium-care-preview-bA8h3L`, PID 2425047. Its frozen executable reported
build `0.1.0+03e258e` and included then-in-progress host source, before the host
package commit `ff55aec`. This is initial integration evidence, not final-build
sign-off. The live shim owner on port 7393 was not changed or given care.

Commands were submitted by actual headless Chromium page JavaScript through
same-origin `fetch`, after verifying `/status` named exactly the copied directory.
This exercised the browser API before the visible care panel was implemented.

- Registration: 200, server-issued client/epoch.
- Feed: 202, seq 1, boundary 68,804; applied 3 material and 6 chemical energy
  over five cells.
- Exact repeat: 200, duplicate true, same seq 1 and applied quantities.
- Reusing the same identity/request with another kind: 409 conflict.
- Rain: 202, seq 2, boundary 68,805, scheduled 4 total depth over 13 cells;
  endpoint 68,925.
- Clean: 202, seq 3, boundary 68,806; partial removal of 1.9568174999175456
  material and 3.6010453428206555 chemical energy.
- Face 5: 400 invalid. New feed inside cooldown: 429. GET mutation: 405.

At tick 68,947 status was ready, no hold, zero outstanding requests and zero
outstanding journal outcomes. The journal used 1,152 bytes of its 4,194,304-byte
limit. Only three admitted receipts existed; the duplicate and rejected requests
did not add ecological actions.

A separate Chromium page served from the candidate gallery on port 7400 tried
the custom-header registration POST to port 7398. The browser rejected the
cross-origin request (`TypeError: Failed to fetch`), confirming the preflight
barrier in an actual browser, not only in Rust string/socket assertions.

Root also ran `cargo test --workspace --offline --quiet` against the committed
phase-1 host package with no failures. This does not discharge subsequent source
review findings or prove real power-loss safety by itself.

## Pending before live care rollout

Finish the visible care panel and its real-browser target/button checks; resolve
the final journal durability review; run the replay/failure-injection tests and
final regression pass. Preserve a fresh exact schema-7 handover checkpoint from
the actual cube runner before migrating it. The shared viewer is already live;
care remains isolated until these gates pass.
