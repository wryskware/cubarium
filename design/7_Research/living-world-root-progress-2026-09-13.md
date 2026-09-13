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

### Independent diagnostic (completed; follow-ups below)

Commit `0c0fd46` adds short-window counter resets (default 200 ticks), compensated
light/heat and receipt sums, care-boundary checks and signed closing ledger
differences. The original persisted-counter gate still fails the command when
exceeded. Observer counts are aggregated across resets rather than silently
reporting only the final window. Four tests pass, including equal ecological
hashes across 1/60/200/420-tick observation windows.

The frozen diagnostic `care_compare-windowed` SHA256 is
`1c13f02d8f355ede833ddedbe7e452dd7de54dea8df9a602ab338aafdffd891d`.
Two fresh seed-1, repeated-care, 12-hour comparisons ran with windows
200 and 20: root exec handles 63946 and 25247, respectively. Outputs are
`seed1-repeated-window200.json` and `seed1-repeated-window20.json` in the artifact
directory. Poll their actual handles; a nonzero strict audit exit now still emits
the full JSON report. Compare both final full state hashes and signed energy
evidence before attributing the original error to rounding. Astra's independent
method and additional narrowing steps are in
`astra-long-run-energy-diagnostic-2026-09-13.md`.

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

## Subsequent verified progress and active handles

The two diagnostic runs completed with full reports and strict exit1 as designed.
Their200/20-tick full hashes match; independent energy error stays below7e-10,
while the persisted heat-counter excess explains the ~2e-5 failure. Seed2's
repeated comparison completed successfully before its chained occasional arm
failed the old pre-report gate. See
[the twelve-hour findings](care-twelve-hour-results-2026-09-13.md), independently
reviewed by Astra in `33be446`. No failed arm was relabeled passing.

The event-driven Unix web accept fix is committed in `8fbabf7`:32 web tests,
three mirror tests and actual A/B Chromium measurements. Distinct pattern frames
rose from41–42/sec to59–60/sec; [full method/results](viewer-cadence-fix-2026-09-13.md).
The two isolated orientation-pattern servers (PIDs2577585/2583198, ports7401/7402)
were stopped after tests; their binaries/results remain. Live cube PID2511131
still reported frozen `c60241f` at tick172095. No live rollout of this fix yet.

Current work to revalidate on continuation:

- Native Fable remains exec52101 / session135e2be2-1019-48c7-ab48-bab462bc4684.
  It revised the multipart draft to one root-owned query after Astra's advice;
  implementation is still incomplete, and placeholders are not delivery. Its
  `living-world-next-progress` records the package boundaries. New review
  `97470f4` contains concrete vertex/filter-tail fixtures; preserve valid art work.
- Native Opus5 high now owns core compensated accounting: exec32477 / session
  f1579c62-fe40-4506-b0c7-94f064f5ab92. Work order
  `accounting-compensation-handoff-2026-09-13.md`; no hunter changes or live work.
  Root committed genuine schema8 tick172800 and+600 fixtures before delegation
  (`3f2900b`). Keep legacy raw additions exactly; append persisted corrections
  and explicit migration. Root must integrate corrected comparison accessors and
  repeat long-run/restart tests after the worker supplies its API.
- Report-preserving old-binary comparisons: seed1 occasional (exec91436,
  `seed1-occasional-window200.json`) and seed3 repeated (exec95652,
  `seed3-repeated-window200.json`). Both run12h with200-tick observer windows.
  They cannot validate the forthcoming core fix; they complete missing baseline
  evidence and help distinguish care/seed effects. Poll actual handles first.

All remaining tickets stay in `animation-roadmap.md`; none of these results
completes growth, visible interactions, autonomy tuning or paid apex ecology.
