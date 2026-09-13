---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Bounded safety review: web listener readiness wait

Read root's working-tree `wait_for_connection` and its caller in
`crates/cubarium/src/sink/web.rs`, including the added shutdown test. No source
or live process changes were made. Root reports 32 web and three mirror tests
passing; this reviewer did not independently rerun them or claim browser A/B results.

No significant correctness or memory-safety issue was identified in the new Unix
wrapper. The stack `libc::pollfd` is fully initialized, the pointer is valid for
one element, and `nfds=1` bounds the OS access to that element. The listener is
borrowed for the whole call and remains owned by the accept thread; this code
does not close or transfer its descriptor during the call. The unsafe allowance
is confined to the wrapper and the stated safety argument matches its use.

The listener remains nonblocking, so readiness does not promise a subsequent
accept will succeed and a stale readiness notification cannot block acceptance.
Poll wakes on readiness; the 100ms value is an idle maximum wait, not a delay
added to each browser request. On a negative result or readiness without POLLIN,
the 10ms backoff avoids immediate retries on ordinary error paths. Interrupted
polls also take that small backoff, which is harmless here.

An idle stop may wait for the current 100ms poll to return before the loop checks
its atomic stop flag; normal scheduling overhead and the error backoff are additional.
The new test allows one second and covers idle and connected shutdown without a
wakeup client. Existing handlers remain independently bounded by permits and
request deadlines; the wrapper does not change their lifecycle. The non-Unix
fallback retains the previous 10ms sleep. These findings support the narrow
readiness change; actual browser latency/frame delivery remains root's A/B task.
