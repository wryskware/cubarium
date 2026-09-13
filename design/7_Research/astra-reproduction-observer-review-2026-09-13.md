---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Reproduction observer adversarial review

**Disposition: validation gates remain before complete-measurement sign-off.**
These are observer defects demonstrated with malformed evidence, not claims that
core currently emits the malformed records or that the ecology has failed.

Read canon and used Lore first, then inspected the new
`examples/hunter_compare/reproduction.rs` and its integration. No worker-owned
module, harness, production test, core, art, or live state was edited. Tests ran
against a mechanically copied observer plus an isolated diagnostic include.

The initial source SHA256 was
`34c6869ef92c40454398533613a11d4310bce9458126db03d3c9d8a048434c39`.
While reviewing, native work committed aggregate compensation in `f4c410a`;
all eight probes were rerun against its source SHA256
`2d0b8ce0a233c754d7bdb1862d643d35fc8aa9e985b238635cb9975d46a0fd6c`,
with the same outcomes. Root integration was committed separately as `71669e3`.

## Proven gaps and narrow fixes

1. **Unknown/stale refusal identities are accepted.** `observe` checks a
   NotFunded wrapper agrees with its own parent and no escrow is open, but never
   checks the parent was a known living hunter at the start of the tick. An empty
   world accepts parent `{slot:12345,generation:999}` and increments its stock-
   refusal count. Require pre-step hunter membership, including the cached full
   ID for a hunter dying this tick; do not substitute mere current-organism
   existence.
2. **Duplicate refusals inflate counts.** Two identical same-tick NotFunded
   records for a real hunter are counted twice. Enforce a bounded per-parent
   funding-decision state for the current batch, so duplicate/incompatible
   refusal/funding decisions cannot be counted. A real refusal on a later tick
   remains a separate observation; no permanent refusal history is needed.
3. **A non-hunter can supply a fake funding/refund cycle.** `funded` obtains
   phenotype from any live organism before consulting its hunter-size cache. A
   real ordinary bystander plus arithmetically consistent Funded/Refunded records
   passes and counts one hunter gestation. The immediate closure evades the later
   open-escrow/membership check. Require the parent in the pre-step hunter cache
   before accepting funding; matching fractions alone does not establish identity.
4. **Closing erases the only same-tick deduplication state.** Duplicate the
   genuine same-tick Funded→Miscarried pair for an actual age-dead hunter, retaining
   one real death in each stream. The audit counts funded=2/miscarried=2 for one
   key and accepts the unchanged closing world. `take` removes the open key, so
   the second Funded appears fresh. Track keys/parents funded and closed in this
   batch independently of the open map. Preserve legitimate Funded→Miscarried,
   reject reopening the same key after closure, and keep scratch-state commit
   atomic on rejection. This requires bounded per-tick state, not lifetime history.
5. **A missing hunter death record is invisible.** Remove HunterEvent::Death from
   the genuine same-tick loss fixture, retaining its LifeEvent::Death and
   miscarriage. `TickIndex` indexes only life deaths and accepts it. Independently
   match the known hunter's death across both streams, full ID and cause, exactly
   once; test missing, duplicate, and disagreeing hunter-death records. The inspected
   outer harness also does not reconcile this second death stream, so its ordinary
   population bookkeeping does not close this gap.
6. **Refunds need a surviving parent.** In the genuine same-tick funding/death
   fixture, replace Miscarried with an arithmetically balanced Refunded record.
   Keep both actual death records and the closing world where the parent is gone.
   The audit accepts refunded=1/miscarried=0. `refunded` checks only numbers and
   the open key, never survival/death facts. A refund must be consistent with a
   live closing parent and no same-tick death; conversely the dying parent's
   outstanding escrow must end as miscarriage, not refund. Check the semantic
   outcome as well as the algebra.
7. **Malformed maximum tick panics rather than returning an error.** A Funded
   key with `started_tick=u64::MAX` reaches unchecked `+1` in `funded` and panics
   in the debug probe, bypassing the harness's normal Result failure path. Use
   checked arithmetic (or a guarded subtraction from the completed tick). Audit
   `self.last_tick + 1` similarly. The demonstrated panic does not mutate the
   observer's scratch-committed state; it is still not the promised clean refusal.

## Proven module limitation already covered by the outer harness

The eighth probe attaches the observer to an empty opening, adds a founder,
steps, and supplies no birth records. The module silently learns the new member
in its size-cache refresh. **Do not report this as a proven full-harness escape:**
root's `Arm::step` separately reconciles living IDs against birth/death events and
each cached lineage's hunter membership against World; the unannounced addition
fails there. Either make this dependency explicit or give the module its own
exact prior-members + births − deaths reconciliation. The stronger pre-step
membership checks needed for findings 1 and 3 should not silently legitimize
unexplained additions on the next tick.

## Evidence and reproduction

Preserved fixture source:
[`astra-reproduction-observer-probes-2026-09-13.rs`](astra-reproduction-observer-probes-2026-09-13.rs).
It is intentionally **bug-demonstrating**: successful assertions establish the
bad acceptance/panic behavior, not a passing observer. Convert these into
rejection/atomicity tests for the fix; retain the genuine-stream positive controls.

The standalone crate is at
`/tmp/cubarium-reproduction-observer-review-I8l3kE/Cargo.toml`, with a copied
`reproduction.rs` and an include of the fixture inside its existing test module.
Its dependencies are anyhow 1, serde 1 with derive, serde_json 1, and path
dependencies on this repository's cubarium-core and cubarium-surface. Run:

```text
cargo test --offline --manifest-path /tmp/cubarium-reproduction-observer-review-I8l3kE/Cargo.toml astra_ -- --nocapture
```

Initial copy: **17 existing tests + 8 diagnostic probes passed**, terminal exit 0.
Final `f4c410a` copy: **8/8 probes passed**, terminal exit 0, reproducing every
listed outcome; the maximum-key panic is deliberately caught by its probe. The
original copy is retained beside it as `reproduction-original.rs`. These results
do not justify the main harness's complete-experiment flag. Re-run the corrected
negative fixtures and the genuine streams before lifting the gate.
