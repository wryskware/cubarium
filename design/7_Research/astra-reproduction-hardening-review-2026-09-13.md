---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Reproduction observer hardening: independent addendum

The eight original probes are closed by `2ac20eb`, but two adjacent malformed
transaction sequences still pass. These are observer validation gaps, not evidence
that core emits such sequences, and not a judgment about hunter biology.
Preserve `bf3407b` and its original diagnostic fixtures as historical evidence.

Reviewed the actual current `examples/hunter_compare/reproduction.rs`, verified
byte-identical to `2ac20eb` by SHA256:
`314a36ec4a2909387588f4300a7770592fcbb14c8a867732371a0739e1b07009`.
Canon was consulted; no production, worker source, live process, or state changed.

## Original findings: closed

| Original malformed evidence | Actual hardening and exercised coverage |
| --- | --- |
| Unknown/stale refusal parent | Full pre-step hunter membership before every transaction; unknown refusal and wrong-generation tests pass. |
| Duplicate same-tick refusal | Per-parent refusal set rejects the second; counts remain unchanged. |
| Ordinary parent with balanced funding/refund | Pre-step membership rejects before phenotype-based arithmetic can legitimize it. |
| Repeated same-key Funded→Miscarried pair | Per-tick funded/closed tracking survives open-map removal; duplicate is rejected, original genuine batch then succeeds. |
| Missing hunter death | Both death channels reconcile full ID and cause in both directions; missing, duplicate, and disagreeing hunter deaths are tested. |
| Extreme started tick | Funding compares against checked completed-tick subtraction; maximum key returns Err, not panic. |
| Unannounced membership addition | Exact previous members − hunter deaths + audited births must equal closing membership. The module no longer silently absorbs the new founder. |
| Refund to dead parent | Refund requires a surviving organism and no indexed death. Relabelled miscarriage is rejected; genuine loss remains valid. The new terminal-branch test also rejects birth to a dead parent. |

Rejection is structurally atomic: open escrows, totals, dispositions and size cache
are scratch state; membership, last tick and observer state commit only after all
reconciliation succeeds. Tests compare summaries, and several retry the genuine
batch on the same observer. This is not a claim that every private field has a
separate exhaustive equality test.

## Two remaining concrete false accepts

1. **A genuine Born followed by NotFunded for its parent is accepted.** The saved
   probe stages an actual due birth, appends a stock-refusal event at the same tick,
   and leaves World and both genuine streams otherwise unchanged. `observe`
   checks refusal against `open` and `seen.funded`, but not `seen.closed`; the birth
   removed the open escrow, so the extra refusal increments the count.
2. **A real hunter can report a fictitious same-pass Funded→Refunded cycle.**
   After a genuine quiet tick with no funding, the probe supplies the existing
   balanced funding/refund helper for that actual pre-step member. Both records
   are accepted, incrementing funded and refunded while closing state has no
   escrow. Membership now correctly rejects ordinary parents, but a hunter still
   exposes the missing disposition check. `refunded`/`close` does not reject an
   escrow funded in this very batch.

These contradict actual core control flow at `world.rs` physiology: `if due`
queues a birth; `else if bud && escrow.is_none()` funds or refuses. Birth/refund is
settled later from the queued list. Newly funded escrows cannot enter that list
in the same pass, even with a zero-duration setting. Death checks follow budding,
so **Funded→Miscarried remains a valid same-pass combination** and must stay so.

Minimal fix: complete the per-parent disposition compatibility checks rather
than add persistent history. Refusal must reject any prior closure. Born and
Refunded must reject same-batch funding. Preserve funding then miscarriage,
ordinary isolated refusal followed by death without escrow, and genuine due
birth/refund. Existing closure→funding rejection already handles that order.
The two saved correctness tests include atomicity and genuine retry assertions
that become reachable after rejection is fixed.

## Exact validation and reproducibility

- Actual repository target: `cargo test -p cubarium --example hunter_compare
  reproduction::tests` — **27 passed, 0 failed, 39 filtered**, exit 0. This includes
  genuine birth, cap refund, prior-escrow miscarriage, same-tick funding/death,
  bounded memory over repeated outcomes, and observed/unobserved 400-tick identity.
- `cargo test -p cubarium --test hunter_observers` — **22 passed**, exit 0. These
  are accounting/recovery tests, **not 22 additional reproduction tests**. The
  example's reported 66 tests include its 27 reproduction tests; do not add those
  counts as independent coverage. Root is separately running that full target.
- New isolated probes: **3 passed, 2 failed**, exit 101. Passing checks cover
  observer `last_tick=u64::MAX` returning an atomic error without panic, a genuine
  newborn forbidden from reporting a pre-step funding decision, and missing or
  duplicated ordinary-death records with successful genuine retries. The failures
  are the two false accepts above.

New fixture:
[`astra-reproduction-hardening-probes-2026-09-13.rs`](astra-reproduction-hardening-probes-2026-09-13.rs).
Unlike the old diagnostic file, these assert the desired rejection behavior.
It is included inside the existing test module of a fresh mechanical copy of the
verified source at `/tmp/cubarium-reproduction-hardening-W7OhNT/reproduction.rs`;
the only copy modification is the include. Its standalone Cargo manifest uses
this repository's actual core/surface and anyhow/serde/serde_json. Run:

```text
cargo test --offline --manifest-path /tmp/cubarium-reproduction-hardening-W7OhNT/Cargo.toml astra_ -- --nocapture
```

Close these small branch-compatibility gaps and rerun genuine streams before
freezing the complete-audit comparison. Source hardening alone does not establish
full 12-seed measurement completion, ledger transfer-site independence, support
for attaching mid-gestation, or biological viability.
