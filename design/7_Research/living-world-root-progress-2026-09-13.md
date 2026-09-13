---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Root: ongoing living-world measurements

The preceding care rollout was concrete progress. The active full-thread goal
remains open; this record does not reduce it to the experiments below.

## Work running separately

Fable 5.1 owns the two production art/rendering packages described in
`living-world-next-handoff-2026-09-13.md`. Its native session is
`135e2be2-1019-48c7-ab48-bab462bc4684`, root exec handle 52101. Poll the actual
handle/process before assuming it stopped; quiet model output is not completion.
Astra's geometry and fixed-hunter plans are committed as `c2fac6a` and `d2b38f9`.
The hunter plan is a candidate implementation recipe, not validated ecology.

## Longer care experiment harness

`care_compare` now accepts an exact checkpoint OR a fresh default seed, durations
through 24 hours, and care-cycle periods from 60 seconds through one hour. Defaults
remain the previous ten-minute run / two-minute care cycle. Each cycle feeds at
its start, rains three seconds later, and cleans at 15 seconds. Spatial targets
rotate between an ordinary face location, side seam, and open rim. There is no
teleport/attraction boost, changed ambient support, or live input.

The harness records all care outcomes, ten-minute census samples, per-form counts,
first extinction and worst inventory-scaled resource residuals. A bounded observer
tracks descendants of opening individuals using complete generation-checked IDs,
resolving births before parent removals. For a fresh seed these are actual founder
lineages; for a loaded checkpoint earlier ancestry is unknown. Four unit tests
cover CLI limits, schedules, slot reuse/dead-parent ancestry and deterministic
observations. Commits `6a77da6`, `ce55d9b`.

Twelve-hour runs use 864,000 ticks, `--care-every 2400` (repeated) or `12000`
(occasional). Each invocation contains its own matched untouched baseline. Art
and simulation work is parallel, so these are not performance benchmarks.

Initial attempts failed their unchanged strict energy residual gate:

- Seed 1 repeated: 0.000020506936266428966.
- Seed 1 occasional: 0.000022022607851113207.
- Seed 3 repeated invocation: 0.00001980965316761285.

The original harness threw before emitting its final JSON, so these errors alone
do not identify which arm failed or establish population survival. The harness
now emits complete numerical evidence and retains an unsuccessful exit status
when either arm fails. It includes limits, opening inventories and cumulative
light/heat, without raising tolerances. Further diagnosis must distinguish
long-lived floating-point ledger accumulation from a real transfer defect;
neither benign rounding nor broken ecological conservation is established yet.

Artifacts/frozen executables live in ignored
`captures/care-longitudinal-2026-09-13/`. The initial binary SHA256 is
`4ea17701733348f7efce23c94f9a31a179fef592054d2d930ed3bddbacead969`;
the ancestry binary is `4772302c184510cfd9df2a6afcc715e9ce31a30581d65a564a356a3b8b3d55c6`.
Do not treat empty JSON files from failed pre-report runs as completed reports.

## Actual browser cadence

Added `scripts/viewer-cadence.mjs`: a read-only diagnostic connecting to an
already-running isolated loopback Chromium debugger. It opens/closes its own
target, counts RAF callbacks, frame responses, actual net draw calls and distinct
render sequences, and reports interval percentiles. It submits no care. Node
syntax and four invalid-argument checks passed. Invocation:

```text
node scripts/viewer-cadence.mjs http://127.0.0.1:7393/ 15 9227
```

Against live PID 2511131/build `c60241f` with the numerical runs active:
host submitted 60fps, RAF 60fps, net drew 60fps, but only **52.93 distinct net
frames/sec**. Distinct-frame gaps had median 16.7ms, p95 33.4ms, maximum 33.6ms;
106 gaps exceeded 25ms. RAF had no such long gaps. This supports investigating
frame-delivery/polling aliasing independently of ecological tick rate and sprite
animation. Source currently polls once per RAF with one request in flight and
a 10ms nonblocking accept-loop sleep; a causal fix still needs a controlled
before/after test. It is not evidence that the cube scans out at 53fps.

Headless instrumentation adds overhead and status boundaries include HTTP latency.
This does not measure Wrysk's own browser or physical display pacing. Report file:
`captures/care-longitudinal-2026-09-13/viewer-cadence-concurrent.json`.

The actual cube remained on its frozen build throughout. A read-only check at
tick 148,039 reported ready/no outstanding care; five user-originated receipts
were present. Root has not submitted additional live inputs or reset that world.
