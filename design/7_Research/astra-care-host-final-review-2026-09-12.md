---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Final bounded host source review at ff55aec

Checked the committed runner/journal lifetime and recovery path against
`root-care-runner-review-2026-09-12.md` and
`root-care-journal-review-2026-09-12.md`. Production files and live state were not
changed. Root's successful browser checks are separate evidence; this disposition
comes from source inspection and does not claim additional fault tests passed.

## Blocker: physical disk exhaustion is classified as safe capacity refusal

`care/journal.rs::uncertain` converts every `io::ErrorKind::StorageFull` into
`JournalError::Full`. `append` invokes it on `write_durably`, which contains both
the configured 4 MiB pre-write guard and the actual seek, `write_all`, and
`sync_all` calls. Real ENOSPC from those calls is therefore classified the same
way as the in-memory configured-capacity guard.

A write can partially succeed before ENOSPC, or a sync can fail after complete
record bytes reached the file. These are uncertain results. Current code does
not poison the writer for `Full`, and `CareRuntime::drain_acks` handles an accepted
`Full` by reclaiming sequence numbers, clearing the hold, and resuming ecology.
Later writes can follow a torn suffix; after restart a surviving complete accepted
record can apply at a boundary the original world passed without applying it.

Separate the two error origins structurally. The advertised-capacity comparison
must return typed `JournalError::Full` before any write attempt. Once seeking,
writing, or syncing begins, every I/O failure, including StorageFull/ENOSPC, is
`Uncertain` and poisons further writes. Do not infer whether bytes changed from
the OS error kind. The same distinction applies to diagnostic outcome writes.

Focused tests: inject ENOSPC after a short accepted-record write, and after a
complete write at sync. Assert the acknowledgement is uncertain, the runner stays
held at the application boundary, no sequence is reclaimed for a new command,
and subsequent queued jobs leave bytes unchanged. Reopen the short-write case as
a repaired valid prefix and the complete-record case as a pending command applied
once at that boundary. Separately preserve the existing configured-limit test:
no write attempt, typed `Full`, unchanged bytes, and continued autonomous ticks.

## Blocker: first journal pathname lacks a directory durability barrier

`Journal::open_with_hooks` creates `care.jsonl` with `create(true)` and syncs the
file when appending the epoch, but does not sync its parent directory. Syncing file
contents alone does not establish durability of a newly created directory entry.
`state::write_snapshot` already uses a parent-directory sync for the same reason.

For a newly created world, the subsequent opening checkpoint happens to sync the
same directory before care opens. A resumed schema-7 world receiving care for the
first time does not take that opening-checkpoint branch. It can acknowledge an
accepted command before the new journal pathname is durably established. Power
loss can then lose the journal entry while leaving the old world checkpoint,
silently losing accepted history.

Establish a successful parent-directory sync after creating the journal and before
allowing durable acceptance. Doing it on every open is also a simple acceptable
implementation. A failure must keep intake closed or fail startup; do not publish
a usable service merely because the file's own sync succeeded.

Focused test: start from an existing valid snapshot with no care journal, inject
a failure specifically at the new journal's parent-directory sync, and assert no
client can receive durable acceptance and no command changes ecology. Also test
the successful first-journal path. A hook records that the directory-sync barrier
occurred before the first accepted append; a unit test need not claim to emulate
actual power-loss filesystem behavior.

## Previous gates visibly addressed in this commit

Existing accepted history is recovered even without `--care`; the flag gates new
intake. The whole pending schedule is checked before stepping and new requests are
gated during replay. Uncertain commits hold a fixed application boundary, and
held presentation uses `f = 1`. Both persistent worker threads retain the shared
OS lock handle for their actual lifetimes, including detached/early-return paths.

Journal reads are bounded, complete malformed final records are refused, only
unterminated suffixes are repaired, and append sizes have a central guard. The
writer owns a poison gate before subsequent writes. Outcome failures report the
actual observed boundary. These fixes address the earlier review mechanisms;
the ENOSPC classification above is the remaining exception to the poison/hold
guarantee. Source inspection identified no additional significant blocker within
this bounded pass.
