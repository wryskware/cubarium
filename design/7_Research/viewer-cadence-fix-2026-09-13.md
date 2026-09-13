---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Viewer request pacing: measured correction

Root's real headless-Chromium diagnostic found repeated/skipped frame delivery
despite 60Hz RAF and host submission. Lore pointed to the web sink's newest-frame
mailbox and fetch loop; source verification found a 10ms sleep whenever the
nonblocking listener had no connection. The browser polls once per RAF, so the
variable accept delay can fetch the same host sequence twice and miss its successor.

The Unix accept loop now waits for socket readability using `poll(2)` rather than
sleeping through arriving connections. It retains a nonblocking listener, bounded
32-handler admission, request deadlines, the same newest-frame bytes and all care
guards. The wait times out after 100ms so idle shutdown does not need a wakeup
client or another file descriptor; resource errors retain 10ms backoff. Non-Unix
uses the old fallback. One localized, documented unsafe call uses the already
present libc dependency. No core, frame encoding, browser loop or shim change.

## Tests and browser comparison

32 web tests and three mirror tests passed with loopback access, including new
idle/partial-client shutdown checks. The initial sandbox run could not bind
sockets; another build caught a transient unresolved export while Fable was
changing its independent renderer API. Both were rerun successfully after their
causes were resolved; neither failure was hidden by reducing test coverage.

Frozen `web_viewer` example binaries ran the same 60Hz orientation pattern on
ports 7401 and 7402, without a world, state writes, or shim output. Measurements
used `scripts/viewer-cadence.mjs` for 15s each in one isolated Chromium debugger.
Runs were ordered before1, after1, before2, after2; background numerical work
finished during this session, so this is not a controlled CPU benchmark.

| Run | Submitted fps | RAF fps | Distinct net frames/sec | Distinct-frame p95 gap | Gaps >25ms |
| --- | --- | --- | --- | --- | --- |
| Before 1 | 60.00 | 60.00 | 41.73 | 33.4ms | 274 |
| After 1 | 60.00 | 60.00 | 59.93 | 16.8ms | 0 |
| Before 2 | 60.00 | 60.00 | 41.13 | 33.4ms | 281 |
| After 2 | 59.93 | 60.00 | 59.40 | 16.8ms | 8 |

Raw JSON and binaries are in ignored `captures/viewer-cadence-2026-09-13/`.
Before SHA256: `0ac42bd40762b99c315450ff7cf3c5a30e3eefefd32a4bb3554b2e38f20e69d2`.
After SHA256: `7de71ff9a2fa8c18e0a8816af2b1638c340a319096fcf5cccb9b2852a7b9fad2`.
The example's build label also reflects contemporaneous documentation commits;
the web-source change between these two builds is the readiness wait above.

The earlier actual-world observation was 52.93 distinct frames/sec, not the
41–42 pattern baseline. Source/browser phase and workload affect the amount of
aliasing. These comparisons support removing the accept-delay contribution,
not promising every browser or physical cube displays exactly60 unique frames.
Instrumentation and network boundaries add overhead; hardware scanout remains
unmeasured. The live cube stayed on its frozen `c60241f` executable throughout.
