---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Integrator review: journal bounds and final-line corruption

Source review of the in-progress host `care/journal.rs`, after the abort path
was removed. These are sign-off corrections within the existing contract, not
new interaction features. Host worker owns implementation and focused tests.

## Preserve malformed complete records

`verify` currently truncates an unparseable final record even when terminated
by a newline. This includes syntactically valid JSON with an unsupported record
type or invalid accepted payload. Such a complete record is not demonstrably an
interrupted append; silently dropping it can discard a scheduled input or a
future-version record while claiming successful recovery.

Only an unterminated trailing byte suffix should use the torn-tail repair path.
If a newline-terminated record fails JSON or semantic validation, refuse recovery
and preserve the bytes even when it is last. Test an unterminated partial JSON
record separately from newline-terminated invalid JSON and a valid-JSON unknown
record type. The conservative refusal preserves evidence instead of guessing.

## Enforce the advertised journal bound on disk

`Journal::open_with_hooks` currently reads the entire existing file with
`read_to_end` before inspecting its size. `write_durably` has no size guard, and
each open unconditionally appends an epoch. Intake's estimate keeps ordinary
commands bounded but does not bound opening an oversized file or repeated epoch
appends after the journal reaches its limit.

Bound the read before allocation (limit plus a small sentinel is enough) and
guard actual append byte lengths centrally, including epochs and diagnostic
outcomes. Account for outstanding outcome reserves when admitting new work.
Capacity refusal before attempting a scheduled write is distinct from an
uncertain write: it must refuse new care without holding autonomous simulation.
At a full journal, still preserve/validate/replay existing history; do not silently
create a new journal or world. If writing another epoch is impossible, disable
new care rather than grow the file or misclassify this as an ambiguous append.

Focused evidence: reopen a journal at capacity without increasing its length;
oversized-file rejection without reading an unbounded allocation; a capacity
refusal that leaves world ticks advancing; existing pending accepted records
still replay correctly. Invalid history can legitimately refuse startup, unlike
ordinary exhaustion of an otherwise valid care journal.
