---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Reproduction observer: branch gaps verified closed

The two remaining false accepts in
[`astra-reproduction-hardening-review-2026-09-13.md`](astra-reproduction-hardening-review-2026-09-13.md)
are closed by `4fe612b`. The original eight probes and subsequent evidence remain
unchanged; this addendum records the new result rather than rewriting history.

Verified actual observer source byte-identical to that commit, SHA256
`a0f8d7ce233d8205dbb6db4e65b3388a21116f1a5bb056d67d3a0559a8899a66`.
Made a fresh mechanical copy, added only the unchanged five-probe research include
inside its existing test module, and ran the complete isolated module:

```text
cargo test --offline --manifest-path /tmp/cubarium-reproduction-closed-6VcMJe/Cargo.toml --target-dir /tmp/cubarium-reproduction-hardening-W7OhNT/target -- --nocapture
```

**35 passed, 0 failed, exit 0:** 30 actual observer module tests plus five research
probes. Three research probes overlap the newly ported production tests; these
are not 35 independent scenarios. This is an isolated observer run against actual
repository core/surface, not a claim that the concurrently edited host suite ran.

Both previously failing probes now reject their malformed batch without changing
the summary and then accept the original genuine batch on the same observer:

- Born followed by NotFunded for the parent is rejected by the new closed-parent
  refusal guard.
- A genuine member's fictitious same-pass Funded→Refunded is rejected by the
  due-closure guard against same-pass funding/refusal.

The maximum observer tick returns an atomic error without panic; a newborn cannot
report a pre-step funding decision; missing and duplicate ordinary deaths remain
atomic rejections with successful genuine retries. All original tests also pass,
including same-pass Funded→Miscarried, ordinary prior-escrow miscarriage, due birth,
cap refund, dead-parent birth/refund rejection, repeated-key rejection, bounded
memory, and the 400-tick observed/unobserved state-identity positive control.

Source review confirms the new Born/Refunded guards follow the existing survival
checks, preserving their error semantics. Calling `take` before the new guard is
safe: it removes only from scratch state, and no observer fields commit until the
whole batch and closing state reconcile. Miscarriage's permitted same-pass funding
path is unchanged. No existing test was removed or weakened in this fix.

**Disposition:** no remaining concrete blocker from this bounded reproduction
observer review to freezing and rerunning the complete-audit experiment. The full
12-seed comparison still needs its own completion/evidence checks. This does not
establish biological viability, independent observation of unreported transfer-site
mutations, or support for attaching the observer mid-gestation. No production,
worker, live process, or state changes were made during this verification.
