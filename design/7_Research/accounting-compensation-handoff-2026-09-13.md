---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Bounded core correction: persisted energy precision

Root delegates implementation to native Opus 5 high. Read repository AGENTS,
canon, `care-twelve-hour-results-2026-09-13.md` and
`astra-long-run-energy-diagnostic-2026-09-13.md`. This corrects a demonstrated
long-run accounting issue, not ecological balance. Do not introduce hunters,
change any biological defaults/operation order/RNG, touch live processes/state,
or edit Fable's renderer/host/art files. Root owns the comparison harness.

Own `crates/cubarium-core/` and a new core correction research report only.
Fable is concurrently editing presentation; preserve all unrelated dirty files.
Use apply_patch for edits and commit validated source/tests/fixtures with explicit
paths under `flock /tmp/cubarium-shared-care/git.lock`. No push. Report actual
commands/results, APIs and remaining integration needs to root.

## Required compatible representation

Keep every existing raw `light_in_total` and `heat_out_total` addition EXACTLY
as before, including order and floating-point result. Append persisted signed
Neumaier correction components to WorldState (a narrowly named accounting
extension), bump snapshot to schema9, freeze explicit schema8 mirror/projection,
and retain schema7 migration. Migrated corrections are zero: no historical
rounding repair is claimed. Existing schema7/8 projected continuations must
remain byte-identical, including care. All nested existing wire types stay fixed.

For every additive amount, with previous raw s and next=s+amount, accumulate
`c += abs(s)>=abs(amount) ? (s-next)+amount : (amount-next)+s`, then store raw=next.
Transient telemetry counters keep their existing arithmetic and reset semantics.
Corrections may be negative; validate finite corrections and finite/nonnegative
combined cumulative totals. Do not silently clamp/reset invalid state.

Expose clear corrected cumulative accessors and a precise delta helper using
`(raw_end-raw_start)+(correction_end-correction_start)`. Core cumulative energy
audits must use the corrected representation; legacy raw fields/hash projections
stay explicit diagnostics, not falsely labeled fully accurate totals. Audit
actual source/update sites exhaustively. Keep care accounting unchanged.

## Evidence before delivery

- Preserve existing genuine schema7 zero-input continuation fixture exactly.
- Root will provide genuine schema8 checkpoint+600-tick continuation fixtures
  produced by frozen `c60241f`; prove migrated projection matches, including care.
  These now exist at `crates/cubarium-core/tests/fixtures/live-v8-172800.cubw`
  and `live-v8-172800-plus600.cubw`: actual live save at172800, continued to173400
  with `captures/checkpoints/shared-care-v8/cubarium run --sink none --state
  /tmp/cubarium-v8-fixture-dlXswQ --speed 0 --seconds 30 --require-resume`.
  Both carry admitted care sequence5 and are decoded/CRC-validated by root.
- Test compensated tiny additions against an independent reference, signed and
  nonfinite validation, migration, snapshot/restart in-flight care and partially
  accumulated correction; uninterrupted/resumed full state hashes must agree.
- Compare stock fields, organisms, weather, config and RNG/legacy projections
  to the pre-change run. This is not authorization to fix tests by dropping
  arbitrary mismatching ecological fields from a comparison.
- Run core tests offline; root integrates the diagnostic harness and repeats
  twelve-hour failures under unchanged thresholds. No live deployment yet.
- Document correction begins at migration, and raw+correction is the persisted
  representation; merely compensating the external observer is not this fix.

If source realities contradict this representation, explain concrete evidence
before changing it. The raw-update-preserving variant was independently reviewed
by Astra and is chosen here to preserve saved-world/ecology compatibility.
