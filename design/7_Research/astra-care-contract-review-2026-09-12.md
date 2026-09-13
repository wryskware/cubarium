---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Independent review of the first care implementation contract

Reviewed `care-contract-2026-09-12.md` as written on 2026-09-12 against the core and
host persistence paths previously inspected in `astra-care-review-2026-09-12.md`.
This is a proposed correction to the implementation contract, not canon. No core,
host, or live-state files changed in this review.

## Blocking: asynchronous admission does not yet replay at the actual tick

The contract assigns `target_tick = current_tick + 1` before the worker's durable
append, allows the world to continue stepping, and permits admission after that
target. It then claims a crash after application and before checkpoint replays to
the same tick. These statements do not hold together.

Counterexample: world at 100; journal schedules 101; slow fsync permits simulation
to reach 105; command actually applies at 106; power fails before outcome is
durable and before a new checkpoint. Recovery from tick 90 sees only target 101
and applies there. Food availability, rain, decomposition, and decisions differ
for five ticks, so exact replay fails even though sequence dedup prevents a second
application to an already checkpointed state. Recording actual tick in an outcome
after application leaves this crash window open.

### Minimal correct protocol: commit at a held simulation boundary

Prefer one durable scheduled-command record and an explicit short pause in tick
advancement when a command commits. HTTP intake and validation stay asynchronous.

1. The simulation owner drains a bounded prepared-command FIFO at a boundary with
   exactly `B` completed ticks. It assigns the next sequence and immutable
   `apply_after_tick = B`. It does not step beyond B while committing this record.
2. The journal worker writes and syncs the command, its sequence, identity, resolved
   payload, and that boundary. It acknowledges durability to the simulation owner.
3. Only on that acknowledgement does the owner call `apply_care` at B, then resume
   normal stepping. The external durable acceptance receipt can now name B.
   An outcome record reports results but is not needed to reconstruct timing.
4. Replay sorts by boundary then sequence. Starting from a snapshot at tick S,
   skip sequences already admitted; apply a pending entry for B when completed
   ticks equal B, before advancing to B+1. An unadmitted entry with B < S is an
   inconsistent recovery state, not permission to silently apply it late.
5. Several commands at one boundary retain sequence order. A snapshot of boundary
   B after some admissions also stores their sequence cursor, so the remaining
   commands at B apply exactly once. Persist active rain as already specified.

Using `apply_after_tick` avoids ambiguity between the last completed tick and the
next tick's number. Another naming convention is fine if application, receipts,
snapshots, and replay use the same definition explicitly.

This protocol makes no zero-input changes and avoids disk I/O on HTTP handlers.
It does cause a brief simulation stall on care admission. The web server can keep
serving its latest frame during that stall. A fixed future lead time does not
remove the need to hold the boundary: disk latency can exceed any chosen lead.
The implementation should not claim nonblocking simulation plus exact actual-tick
replay until it supplies a stronger protocol proving both.

### Error case: failed fsync is not proof that a record is absent

After an append or sync error, some or all of the scheduled record may survive.
If the running world skips the command and advances past B, but recovery later
finds that record, it applies a command the original history never applied.
Consequently the contract's unconditional “disable care and keep the world
running” is unsafe once a scheduled commit has an uncertain durable result.

Failures before attempting a scheduled write can reject care and keep running.
After an uncertain commit write, hold tick advancement at B and report the error,
or terminate cleanly at B with an error. Recovery can then apply a surviving
record at the same boundary without rewriting already-executed ecology. A durable
abort record could permit continuation, but only if its durability is established;
that is not a fallback when the disk remains unavailable. A timeout is likewise
not proof that the worker failed to commit. Keep acceptance responses separate
from “request received” status and do not acknowledge durability prematurely.

A torn final journal line must be removed/truncated back to the verified prefix
before a later append, with the prefix preserved. Merely ignoring it while
appending new records turns the torn suffix into interior corruption. A complete
record that survived an interrupted sync is recoverable even if the client never
received its receipt; retry identity must handle that ordinary uncertainty.

## Blocking: PID liveness is not exclusive state ownership

The proposed `LOCK` file with a `/proc` PID check has a check-then-create race:
two launchers can both observe an absent/dead owner and begin writing. PID reuse
also makes liveness insufficient to establish that it is the same process.
An OS-backed exclusive file lock, acquired atomically and held by an open handle
for the entire runner/checkpointer lifetime, is the narrow reliable mechanism.
The PID/start stamp can be diagnostic metadata inside the locked file. Do not
unlink or replace the lock inode while an owner may still hold it. Ensure lock
acquisition precedes fresh-occupancy checks, loading, and all persistent writes.
Release only after the checkpoint worker has stopped. A stale file itself is
harmless when the OS lock was released on process exit.

## Required clarification: journal and identity history bounds

The intake, outstanding count, receipts, and duplicate payload window are bounded,
but an append-only journal grows indefinitely. Restoring only 256 accepted records
cannot reconstruct a last-seen request number for every client that ever existed.
Forgetting an older client's high-water mark then allowing that same identity to
submit again turns a previously applied request into a new command.

For this first slice, a documented hard byte/record limit is a reasonable journal
bound: refuse further care before reaching it while continuing autonomous life.
Reserve enough space for any mandatory records belonging to commands already
accepted. Show the limit in care status. This avoids designing compaction now.
If implementing compaction, coordinate a durable checkpoint cursor, every retained
fallback snapshot, pending commands, and dedup state before dropping journal data;
trimming merely to the latest 256 outcomes is not safe replay retention.

Bound the number of registered active clients and require server-issued identity
epochs. Persist each active client's high-water mark and the recent receipt window.
Unknown or retired identities must be rejected, not implicitly re-registered by a
POST. A durable monotonic epoch rollover can retire old client identities in bulk
without an unbounded tombstone set; new registration issues distinct identities.
Refreshing the browser's current transport token must not change an old request's
identity into a new one. A simpler alternative is a finite documented service
session and client cap that rejects new registration once full, with an explicit
epoch rollover on restart. State the retry behavior across that rollover honestly.

## Required clarification: the rain dose has two incompatible meanings

The table calls `RAIN_DEPTH = 0.9` the integrated centre-cell depth, but the formula
is `rate_c = RAIN_DEPTH * normalized_weight_c * envelope`, where both temporal
and spatial weights normalize to one. That formula delivers 0.9 summed depth over
the whole footprint, and only `0.9 * weight_center` at the centre. It does not
deliver 0.9 there. Choose the summed-footprint interpretation to keep a bounded
total dose independent of seams and rim geometry, and correct the table and
receipt label accordingly. If centre depth is desired instead, remove spatial
normalization and explicitly accept a footprint-dependent total dose.

Define exactly 120 discrete samples (`k = 0..119`), their raised-cosine expression,
and their normalization sum. “k = 0..RAIN_TICKS” is ambiguous between 120 and 121
samples. Floating-point weights sum within a stated tolerance; “exactly 1” should
mean the discrete intended dose, not an unproved bitwise equality. Book actual
per-cell floating-point additions in the source ledger.

## Focused evidence needed before integration

- Inject an intentionally delayed commit acknowledgement so the simulation tries
  to pass its reserved boundary; confirm it stays at B and later applies once.
- Crash after a durable scheduled record but before application, and after
  application but before any outcome record/checkpoint; replay from an older
  snapshot must match uninterrupted ecological state at the same final tick.
- Simulate uncertain sync failure with a complete record visible on reopen;
  verify the runner never advances ecology beyond the unresolved boundary.
- Launch two processes concurrently into one empty state directory; exactly one
  must gain ownership. Killing that owner should make the OS lock available again
  without deleting/replacing the lock file.
- Exceed the duplicate window and journal/client bounds, restart, then retry an
  old request identity. It must either return its retained receipt or reject as
  stale/retired, never produce another ecological application.
- Show per-cell and summed manual water delivery for ordinary, seam, and rim
  targets against the single chosen meaning of the rain dose.

These are contract corrections needed before trusting the requested crash/replay
and exclusive-owner guarantees. Visual care effects can proceed independently
while the host adopts the corrected admission and recovery protocol.

## Revision 2 follow-up: abort recovery remains incomplete

Reviewed `care-contract-2026-09-12.md` revision 2 at commit `6b92feb`. Held-boundary
admission, the OS ownership lock, and the total rain dose resolve the earlier
central issues. The newly introduced abort-and-resume path still has two concrete
crash-consistency holes.

**Partial accepted line followed by abort.** An append error can leave a prefix
of an accepted JSON line without its newline. Appending an abort then produces one
malformed concatenated line. Successful fsync of that append does not make it a
valid durable abort. The runner could void the command and advance, but restart
would reject the resulting interior corruption. Repairing a torn tail only when
opening the journal, as currently specified, does not cover this same-process
error path.

**Missing accepted record in a reserved batch.** Suppose the runner reserves
sequences 20 and 21 at B. Only accepted 20 reaches the file; the next write fails.
The proposed recovery writes aborts 20 and 21, then voids both in memory and
continues. Crash before checkpoint. Replay groups only accepted records, so it
finds 20 but never 21; moreover `{"rec":"abort","seq":21}` does not identify B.
The live cursor reached 21, but recovered cursor reaches only 20. The next accepted
command 22 cannot apply contiguously. A complete accepted record is therefore not
a valid prerequisite for recovering an abort.

### Recommended narrow correction

For this slice, remove abort-and-resume after an uncertain write. Hold at B with
care failed and permit clean stop at B. On restart recover the valid journal
prefix, discard only a torn final suffix, and apply whatever scheduled records
survived at B. Client acceptance can be uncertain, but ecology never progressed
beyond the disputed boundary, so this does not rewrite executed ecological
history. Reserved sequences absent from the surviving journal were never applied;
the restarted process has a new client epoch and can allocate after the surviving
maximum. Do not durably consume sequence numbers for missing records.

This is simpler than adding another transactional record type while implementing
the first care feature. It is an explicit availability tradeoff: a disk failure
can pause the world until recovery or restart.

If abort-and-resume is retained, it needs all of the following as one protocol:

- A single journal owner must inspect and repair the ambiguous write suffix before
  further append. Preserve every complete valid record, truncate only the torn
  final suffix, and establish successful repair. An interior malformed record or
  failed repair keeps the world held. Never blindly append abort to unknown bytes.
- Each abort is self-contained, at least `{seq, apply_after_tick:B}`; a batch abort
  may instead name B and an explicit contiguous sequence range. Persist aborts for
  every reserved sequence before acknowledging that the whole batch is voidable.
- Replay builds the union of accepted records and aborts, not merely accepted
  records with an abort filter. An abort with no accepted counterpart still voids
  its sequence at B. Both records for a sequence must agree on B. Identical abort
  retries are idempotent; conflicting boundaries or accepted payloads are fatal.
- A crash during abort retry can leave any complete prefix. That prefix must be
  replayable without missing sequences: surviving accepted records without aborts
  apply, surviving aborts void, and no ecology has advanced past B before the full
  abort acknowledgement. This outcome uncertainty must be reported honestly.

### Other small recovery clarifications required

Validate the complete pending replay schedule before stepping: sequence continuity
after the snapshot cursor, nondecreasing boundaries in sequence order, and no
unadmitted boundary before the snapshot tick. Sorting malformed input by boundary
does not establish these properties. An unexpected core sequence/boundary rejection
is a recovery error; never silently skip it and resume stepping.

While replay still has future historical commands, new HTTP care must not allocate
from only the snapshot's admitted cursor. Those sequences are already reserved in
the journal. The simple first implementation refuses new care as “replaying” until
the recovered schedule is exhausted, then allocates after the journal maximum.

Require a successfully durable opening/resume checkpoint before enabling care in
a newly created world. Otherwise a crash before the first periodic checkpoint
leaves accepted commands but no persisted original config or initial world to
replay them against. A journal-only directory must not silently create a world
from today's defaults or changed CLI configuration.

The crash test phrased “before accepted fsync: no application” needs to allow a
complete accepted record that survives a crash before fsync returns. The guarantee
is no application before durable acknowledgement in the original process; recovery
may apply a surviving valid record at B even if the client never saw acceptance.

Add injected short writes (mid-JSON and between two batch records), a crash after
only some aborts if retaining them, duplicate abort retries, and a new HTTP request
during replay. These target the actual uncertainty windows above rather than only
clean successful journal writes.
