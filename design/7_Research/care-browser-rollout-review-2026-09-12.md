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

## Initial rollout gates (subsequently completed below)

Finish the visible care panel and its real-browser target/button checks; resolve
the final journal durability review; run the replay/failure-injection tests and
final regression pass. Preserve a fresh exact schema-7 handover checkpoint from
the actual cube runner before migrating it. The shared viewer is already live;
care remains isolated until these gates pass.

## Panel source-review corrections before sign-off

In the first panel implementation, `register()` can return 503 while historical
care is replaying, leaving `careClient` and `careEpoch` null. Once status becomes
ready, `pollCare` never retries registration (its only registration branch
requires a previous non-null epoch). The controls therefore remain disabled
until the page is manually reloaded. Retry registration when the host transitions
to ready and no client exists, with an in-flight guard and bounded retry cadence.
Handle a page opened against a temporarily disabled/offline host similarly; do
not spin registrations each frame or race duplicate initial registrations. Verify
an initial replaying response followed by ready enables the buttons without reload.

`rowFor` limits DOM rows to 24 but never removes their entries from `careRows`,
so the Map retains every removed element and grows with clicks, including
cooldown-rejected requests. Bound the Map as well as the DOM. Verify more than
24 request rows keeps both bounded and retains the newest useful receipts.

These are frontend state/lifetime fixes, not changes to ecological admission.

## Final build verification

The panel corrections landed in `c60241f`. Journal durability corrections,
including uncertain ENOSPC and the initial parent-directory sync barrier, landed
in `30c5ed6`. Root's final full workspace run passed 774 tests, zero failures,
11 ignored (manual captures/long runs). Fable independently reported a passing
full workspace run after its workers finished and committed their packages.

Root tested the final release executable `0.1.0+c60241f` in actual Chromium on
the copied world at port 7398. All five face targets mapped correctly; blank net
space was ignored; the targeting overlay did not change encoded frame bytes.
Real panel buttons applied feed at tick 86,684 (3 material/6 energy), rain at
86,687 (4 depth through 86,807), and partial clean at 86,696 (1.7774457356296849
material/3.3170992266532404 energy). At 86,705 the service was ready with no
outstanding inputs. The panel starts collapsed. Screenshot inspected:
`/tmp/cubarium-care-panel-verified.png`. These checks supersede the worker's
earlier limitation that browser click mapping had not yet been exercised.

## Live recovery and care rollout

Before root's planned graceful handover, the old shared-viewer process PID
2359616 had already exited: port 7393 refused connections and its exec session
reported exit 143. The guarded stop command never reached `kill`, because the
preceding status request failed. The cause of that earlier termination is not
established. There was no final shutdown snapshot; unsaved progress after the
last periodic snapshot may have been lost.

Root validated the newest saved current-world snapshot, `state/world-129600.cubw`,
with both preserved schema-7 and current decoders. Both report tick 129,600,
population 102, and identical ecology hash `14075524997681159629` (the old full
state hash). It was copied outside retention to
`captures/checkpoints/shared-care-v8/world-129600-v7.cubw` before restart.
SHA256: `261dc07095fc4df3ef1cd9d93bac893b2eb2bc0ebc11e7a6ac24d69e0d0ef19c`.

The exact browser-tested executable was frozen at
`captures/checkpoints/shared-care-v8/cubarium`, SHA256
`87de9130fd4d74e08fb51ec6c00086cd38151f31d1f97044bb83fc0365255eec`.
It resumed that checkpoint, without `--fresh`, using:

```text
captures/checkpoints/shared-care-v8/cubarium run --art assets/atelier --sink shim --mirror-web --web-port 7393 --state state --fps 60 --speed 1 --care --require-resume
```

At tick 130,188 the real browser confirmed PID 2511131, build `0.1.0+c60241f`,
sink `shim`, absolute repository `state/`, resumed snapshot 129,600 and start tick
129,600. Care was enabled and ready, with zero receipts/outstanding requests and
only the 75-byte journal header. The panel was collapsed and the live image was
visually inspected (`/tmp/cubarium-care-live-verified.png`). Root submitted no
live care: the user's first input remains theirs. The viewer and shim consume
the same encoded frames, though their display instants can differ.

The first subsequent periodic checkpoint `state/world-130800.cubw` passed full
decode/CRC validation: schema 8, build `0.1.0+c60241f`, population 105, ecology
hash `7043493590478046473`. Every care ledger and admitted sequence remained
zero. A later read-only status at tick 131,950 still reported ready with no
receipts or outstanding work.

Rollback requires the preserved schema-7 executable and this backed-up v7
snapshot in a NEW separate state directory, not an old binary pointed at the
migrated schema-8 `state/`. Recovery instructions accompany the ignored artifacts.
Neither short matched numerical runs nor these browser/replay checks establish
long-run balance under repeated care. Hands-on autonomy tuning, stronger readable
activity responses, wider authored growth and production megafauna remain work
ahead. Cleanup currently exports unrecycled litter and its chemical energy, not
a separate toxin pool.
