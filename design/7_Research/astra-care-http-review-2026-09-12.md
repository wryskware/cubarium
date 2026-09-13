---
design_status: exploration
last_reviewed: 2026-09-12
decision_refs: []
---

# Bounded review of care HTTP and intake

Read `crates/cubarium/src/sink/web.rs` and `crates/cubarium/src/care/mod.rs`
while runner integration was still in progress. Production files were not edited,
and this review did not test the live viewer or revisit journal abort recovery.

## Actionable: connection threads are unbounded

`web.rs::accept_loop` creates a detached OS thread for every accepted connection.
There is no connection permit or active-handler limit. The four-outstanding-command,
64-client, and eight-entry-intake limits apply only after reading the request, so
they cannot bound these threads. Opening many incomplete connections consumes
threads without registering a client or submitting a care command. This structure
predates care, but remains relevant to the bounded HTTP surface now being delivered.

`CONN_TIMEOUT` is a per-read socket timeout, not a total request deadline. A peer
can send a byte shortly before each five-second timeout and retain its thread for
hours while filling the bounded header/body. Consequently it does not establish
the “short-lived thread” property asserted beside the accept loop.

Use a nonblocking fixed connection-permit limit (or a bounded worker pool), releasing
the permit on every exit path, plus an absolute deadline for header/body reception.
Refuse/drop excess connections without spawning another handler or waiting in the
accept loop. Keep this separate from the legitimate five-second care acceptance
wait after a complete request. Verify that excess partial connections do not
increase active handlers past the cap and permits become available after timeout.

One small parser bound correction fits the same change: `read_head` returns when
it finds the delimiter before checking `MAX_REQUEST_BYTES`. A delimiter arriving
in the read that crosses 8 KiB currently admits an oversized head. Check the
resolved header length before returning; this is a small overshoot, not unlimited
header allocation.

## What the inspected intake already bounds correctly

Request bodies require Content-Length, reject Transfer-Encoding, and cap declared
length at 4 KiB. Mutations require the custom header, JSON content type, a recognized
loopback Host with the bound port, and a recognized Origin when supplied. GET
mutation is refused and no CORS preflight is enabled. No browser cross-origin
mutation bypass was identified in this pass. The Origin allowlist also accepts
HTTPS spelling and either loopback hostname alias; that is broader than exact
origin equality, although the custom-header preflight still blocks the corresponding
cross-origin browser request. An exact HTTP-origin check would better match its name.

Client registration stops at 64, rows at 64, outstanding requests at four, and the
prepared channel at eight. Submission checks retained duplicates/conflicts before
cooldown or capacity, preserves the client's high-water mark when rows expire,
and changes counters only after successful enqueue. The condition-variable wait
releases the shared mutex and has a five-second deadline; no mutex/condition-variable
deadlock was found in that path. Cooldowns are global per kind within this service
instance and measured from successful intake; they initialize empty in a new
service. These are observations of the implementation, not a new persistence claim.

The journal acknowledgement channel is unbounded, so joining its worker does not
deadlock on a full acknowledgement channel. `JournalWorker::submit` does use blocking
`SyncSender::send`, despite the comment claiming no channel blocking; its safety
depends on the runner maintaining the stated one-batch-at-a-time discipline.
`shutdown` joins until disk work finishes. Neither is evidence of a deadlock in
the unfinished runner; no runner changes or additional lifecycle guarantees were
assumed by this bounded review.
