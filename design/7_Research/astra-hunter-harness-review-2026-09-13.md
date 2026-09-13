---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent six-arm orchestration review

Read the in-progress `crates/cubarium/examples/hunter_compare.rs` and
`hunter_compare/audit.rs`, plus the actual control initializer and event types.
This is a bounded read-only source review; root owns the implementation and its
running smoke tests. No core, harness, art or live state changes were made here.
The known unavailable exact capture-position and funding-transaction measurements
are explicitly reported by the harness and are not new findings.

## 1. Offspring events are not reconciled one-to-one

In `Arm::step`, construction of `children` deduplicates `HunterEvent::Offspring`
into a `BTreeSet` without rejecting duplicates. Birth processing requires at least
one correct parent/child event, but the later hunter-event loop increments
`offspring` for every event and only checks existence of a matching child birth.

Consequently, two identical offspring events for one actual LifeEvent birth pass
all current checks and report two offspring. A correct event plus another event
with the same child and an incorrect parent also passes, since the good event
satisfies the earlier `any` predicate. This is a real measurement defect even
though the current core may emit the right events: the observer purports to
independently check exactly the behavior whose count can be inflated.

Before updating counters, require unique offspring child IDs and exact parent/child
matching to the unique life birth; reject duplicate/wrong-parent records. Keep the
existing prior-escrow linkage. Fault tests should provide one real hunter birth
with (a) duplicated correct offspring events and (b) one correct plus one
wrong-parent event, and require failure rather than an offspring count of two.
Also check event tick equals the step's committed tick when centralizing this
validation; current matching does not establish temporal correspondence.

## 2. Failed-step summaries can combine different observation boundaries

`Arm::step` first advances World, then checks invariants/audit, then processes
life/hunter events and updates lineage/recovery/occupancy. A failure after the
advance can leave all or some observers behind. `Arm::finish` nevertheless combines
the advanced world's closing tick, snapshot and hunter count with
`closing_population = self.live.len()` and potentially stale or partially changed
observer counters. There is no explicit last fully observed tick or marker on
those individual statistics.

The shared `technical_failure` reason and unsuccessful command already prevent a
false overall pass; this finding is not a claim of silent exit-0. The risk is
misreading the failed arm's closing census and partial statistics as observations
of its saved closing state. Record `last_complete_observer_tick`, source closing
population directly from World, and identify failed-step observer statistics as
partial/untrusted beyond the last complete boundary. Do not silently roll back the
world or count an incompletely processed step as completed observation.

Fault test: force an audit/invariant failure after a world advance with a birth or
death, then assert the actual closing state population is reported accurately and
observer coverage does not claim the failed tick. When a sequential six-arm step
stops midway, retain each arm's actual closing tick rather than implying a shared
matched final instant; current output already includes per-arm world ticks.

## 3. Initialization/finalization errors bypass the retained failure summary

The main loop catches `Arm::step` errors, but both `Arm::new` and `Arm::finish`
are collected with `collect::<Result<_>>()?`. An initializer numerical failure
therefore returns before the failing arm's AuditReport is written or a seed/run
termination record is emitted; previously constructed arms can be left with only
opening files. A finalization failure similarly exits without the intended global
failure summary and prevents finalizing later arms.

Preserve a small run/seed failure record with stage, arm, actual tick if available,
and error whenever the output medium still works. For an initializer audit failure,
write its audit evidence before discarding the partially constructed Arm. A genuine
disk-write failure may make further output impossible and must remain stderr plus
nonzero exit, not an invented guarantee of durability. Fault tests should inject
an initializer receipt mismatch on a writable directory and verify a terminal
failure artifact, then inject a finalization error and verify earlier completed
evidence remains attributable and the run never looks complete.

## Sound parts of the current slice

The loader requires all twelve prescribed seeds, checksum-valid schema 9 age-two-
hour openings, no care/hunter history, matching populations/ecology hashes and
population-rank strata. Every arm clones the same opening. The six profiles retain
separate specialist/facultative disabled controls and fixed placements. The 72h
tick bound is explicit, ancestry uses full IDs and only living entries, and prey
are identified by nonmembership rather than form.

Inventory reads include ordinary bodies/escrows and additional gut M/Q once. The
common pre-import audit retains actual initializer receipts, original inventory-
scaled limits, signed/nonfinite peaks, corrected and independent energy, and raw
legacy diagnostics. The current core control initializer puts spilled heat in the
persisted compensated ledger but not transient telemetry; `Audit::initialize`
adds that receipt heat once to the independent observer, correctly matching this
actual implementation. Its explicit cap-heat test covers that otherwise easy
double-counting omission. Care or unreceipted extra imports fail the audit.

Whole-recovery observations are tickwise and partial measurement status is honest.
Saved executable bytes/SHA256, source cohort checksums, full config, profile SHA256,
post-import snapshot hashes and closing hashes provide substantial reproducibility
metadata. None of these source checks or root's short smoke runs establish paid
predator balance, local prey recovery or exact reproduction funding. This reviewer
did not rerun the concurrently executing harness test suite or claim those results
as independent execution.
