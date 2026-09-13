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

Work handles recorded at that checkpoint (superseded by the update below):

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

## Latest verified integration and running work

Root now integrated the first care-response flourish into the shared frame
(`0324098`), following Astra's module and cleanup-only palette refinement
(`e84df49`, `3112ae1`). Nine runner tests, eight module tests and eight care replay
integration tests pass; the ignored native capture test was also run explicitly.
See [implementation and actual-world visual review](care-flourish-integration-2026-09-13.md).
This is receipt feedback, not the full biological response ticket, and not live.

Opus accounting exec32477 is terminal exit0, delivered `b47eacc`. Root reran the
entire core suite:208passed/2ignored. Genuine schema7/8 continuation and schema9
mid-shower restart tests pass. Root integrated corrected interval readouts and
independent/legacy reports in `57408c4`; five harness tests pass. Astra's additional
source review found no rollout-blocking defect in its initial report. No historical
ledger repair is claimed, and the live binary still cannot read schema9 snapshots.

The frozen corrected diagnostic is
`captures/care-longitudinal-2026-09-13/care_compare-compensated`, SHA256
`9f317241c96865c13a8532eab2c76fc47fc9737843b484e76f212e97ae0058e6`.
Built against core `b47eacc` and harness `57408c4`; concurrent art edits are not
used by this in-memory numerical example. New matched12h jobs, to poll by handle:

- Seed1 repeated: exec41258 → `seed1-repeated-compensated-window200.json`.
- Seed3 repeated: exec74507 → `seed3-repeated-compensated-window200.json`.

Both use864000ticks, care-every2400, audit-window200. Main acceptance uses the
persisted corrected AND independent checks under the original fixed limits;
raw failures remain separately reported. Compare ecology hashes and raw counters
against the old runs, not schema9 full hashes against schema8 full hashes.
Old diagnostic seed1 occasional and seed3 repeated jobs are both terminal exit1
with complete reports, independently documented in `aaa730b`; old empty reports
are not evidence. Their form losses reinforce the remaining prey-recovery gate.

Fable's first orchestrator exec52101 terminated at its600second background-worker
ceiling before delivery; root verified its PID gone and resumed the SAME native
session135e2be2-1019-48c7-ab48-bab462bc4684 with
`CLAUDE_CODE_PRINT_BG_WAIT_CEILING_MS=0`. New active exec69674; filtered output is
`/tmp/cubarium-fable-resumed-2026-09-13.jsonl`. It is recovering/completing the
Lanternjaw renderer/study and authored plant package, not finished merely because
source files or tests now exist. Preserve its owned files and poll before restart.

No new live care, state replacement or deployment was performed in this slice.

## Subsequent completion and next experiment

Corrected repeated-care12h jobs41258/74507 are now terminal exit0. All four arms
pass the original fixed limits with corrected energy peaks below3.6e-10 and
independent peaks below6.5e-10. Exact old/new comparison preserves every receipt,
care ledger, raw flow/residual, census, demographic/ancestry/extinction metric and
final telemetry except the expected schema9 full hash. The ecology hashes remain
identical. Full evidence: `corrected-care-twelve-hour-results-2026-09-13.md`.
Occasional-care followups now running: seed1exec98612 and seed2exec77787, same
frozen binary,864000ticks, care-every12000, audit-window200. Poll before claims.

Root prepared ALL12default prey seeds at exactly2h with a frozen pre-hunterv9
runner and archived all12initial tick-zero worlds separately. Preparation67729
is terminal exit0; all24snapshot identities validated. Populations78–101, eight
seed endpoints without skimmers. No filtering or resampling. Exact paths, strata,
tool tests and proof are in `hunter-preparation-progress-2026-09-13.md`.

The next core package is actually delegated to Opus5high: exec9109, resumed
native sessionf1579c62-fe40-4506-b0c7-94f064f5ab92, output
`/tmp/cubarium-opus-hunter-core-2026-09-13.jsonl`. It implements opt-in paid
Lanternjaw hunting/escape/digestion/singleoffspring in schema10, off by default.
Genuine schema9+600tick fixtures were frozen/committed BEFORE it began (`1f0fc3a`).
Root owns the pending six-arm harness, Fable still owns productionart integration.
Astra contracts `5ad6bbc`/`2c8e90d` define experiment/recovery accounting and flag
the6pxcontact placeholder as visibly wrong: actual grasp is~13.28pxforward.
This must be resolved with sensing/phase/scale, not hidden by shortened artwork.

Fable's side-family growth/topreed packages are committed5d7ea69/b8b8a11. Its
Lanternjaw package/progress note is still being finalized; in-progresscore schema10
edits can temporarily block whole-workspace builds. Do not assume uncommitted
files are its final validated delivery or overwrite them. Root inspected actual
`captures/lanternjaw/gallery.png` and `grounds.png`: recognizable chosenbody,
clear articulated progression and alpha composition over distinct backgrounds.
Those are art studies, not captures of biological hunting or physicalcube tests.
