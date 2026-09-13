---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Adjustable care dose: implementation handoff

Scope: fulfill the owner's requested independently tunable input dose while
preserving the existing autonomous default. This extends the implemented
[care contract](care-contract-2026-09-12.md), not the canon ledger. Ambient
support tuning is a separate outstanding package, with matched ecology evidence
required before adopting a more hands-on preset. No animation-speed multiplier,
free reproduction, globally attracted animals, or new toxicity system.

## Dose semantics

Represent a dose as integer `dose_permille`, 250 through 2000 inclusive;
1000 means the current standard dose. This is a bounded total multiplier, not
per-cell multiplication or repeated queued commands. An omitted field in an
old HTTP request or old journal record means exactly 1000. Explicit null,
fractional, negative, overflowing, zero or out-of-range values are rejected,
not clamped or mistaken for omission. Integer identity avoids float-equivalence
ambiguity in duplicate detection.

The web panel initially offers Gentle (500), Standard (1000, default), and
Generous (1500). The typed API may admit the whole documented integer range.
These are proposed interaction settings, not ecological balance claims.

- Feed: scale the nominal 3 material units and corresponding chemical energy.
  Keep the existing 30-unit net allowance and actual source accounting. A dose
  that exceeds remaining allowance is refused rather than silently reduced.
- Rain: scale the nominal 4 total depth units, keep the same footprint and
  eased 120-tick envelope. Persist the chosen dose of an active shower so
  restart delivers exactly its remaining samples. Report scheduled and delivered
  amounts honestly; changing a UI selection cannot change an existing shower.
- Clean: scale the nominal 2-unit maximum export but retain the 50%-per-cell
  limit, footprint, proportional chemical-energy export and partial receipt.
  Only actual material removed restores the feed allowance. It remains litter
  cleanup, not sterilization or removal of mineral nutrients/organisms/plants.

Keep current cooldowns, queue/client/journal bounds, simulation-boundary
admission, uncertain-write hold, single-owner rules and security checks. Dose
is part of the full request identity: same request ID and different dose is a
conflict; omitted and explicit standard are the same semantic payload.

## Durability and compatibility

Core is the sole authority applying doses. The host validates and journals the
exact amount before application; status and receipt rows expose the amount so
the viewer can explain what was actually requested/applied.

A shower shape change requires a new snapshot schema with explicitly frozen
legacy care/shower mirrors. Current schema is 11, and versions 8/9/10 refer to
the old care shape too; do not silently change their nested postcard layout.
Preserve supported legacy imports, including the intentional refusal of active
schema-10 hunters, and migrate legacy in-flight rain to standard dose. Genuine
pre-change executable fixtures must establish migration and continuation,
including unfinished rain and a schema-11 world with actual hunter state.
Do not fabricate a new struct and label its serialization an old fixture.
Legacy projection helpers must not silently pretend a nonstandard active shower
is representable in the old format: either define an explicitly documented
projection that omits new semantics solely for comparison, or refuse lossy export.

New dose-bearing journal commands need an unmistakably new record discriminator
(for example `accepted_dose_v1`), not merely an extra field the old reader
ignores. The new reader understands both legacy standard records and new
explicit-dose records. Unknown/malformed versions fail closed, including at
the final newline-terminated record; only an actually torn tail is truncatable.
Keep existing journals append-only; do not rewrite history or infer a changed
dose from a receipt. Test mixed old/new journals and complete schedule validation
before advancing a recovered world.

No live rollout in this package. A later upgrade needs a matched snapshot/journal
backup and guarded exact resume; executable-only rollback is unsafe after new
snapshot or journal shapes have been written.

## Ownership and verification

Native Opus worker owns core care/snapshot migration plus host typed command,
journal, runner conversion and HTTP/status plumbing, with relevant tests and a
new progress report. Root owns viewer `src/sink/web/index.html` and browser
acceptance after the wire contract is available. Fable owns the separate
Lanternjaw presentation-boundary package; do not edit its presentation files.
Do not change hunter biology or the frozen running experiment binaries.

Required evidence: explicit default-dose and zero-input legacy continuation;
bounded values and malformed inputs; exact food/energy/water and export ledgers;
seams/rims and sparse/empty cleanup; ongoing rain save/resume at nonstandard
doses; duplicate/conflicting dose identity; queued durable acknowledgement and
post-acceptance restart; mixed journal versions; old executable refusing new
records; snapshot migration against genuine pre-change artifacts. Existing care
durability/security tests must continue passing. Use copied or synthetic worlds.
Check in scoped tested changes through the shared Git lock; no push, no changes
to `state/`, live owner, shim, or `.vscode/`.

This package does not complete ambient-support tuning, ecological response
diversity, quiet habits, long-run predator viability, or lower-priority LCD detail.
