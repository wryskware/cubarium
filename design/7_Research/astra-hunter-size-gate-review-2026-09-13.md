---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent size-gate implementation and seed-1 pilot review

The production mechanism matches the single-change proposal. The unchanged
12-seed comparison remains informative after the existing ledger integration and
the narrow corrections below. This is not biological acceptance: the pilot has
partial juvenile growth, no adult descendants, and worse hunter lineage outcomes.

## Frozen scope and evidence

Reviewed [proposal](astra-hunter-size-aware-growth-proposal-2026-09-13.md)
(`61921e3`) and [handoff](hunter-size-gate-handoff-2026-09-13.md) (`02b5823`),
following canon, Lore and Graft. Source review and Rust tests used an archive of
isolated head `9f4cf7d` (implementation `8390cdd`, baseline `0d867f1`), under
`captures/build-cache/astra-size-gate-review/source`. The native worker's newer,
uncommitted `flow.rs` and `hunter_compare.rs` are **not** certified by these tests.
No production file, live process or original capture was changed; no ecology
experiment was started.

- Original focused Rust suite: **16 passed**, including positive four-cap
  fixtures, strict threshold, charge80 retention, restart and observer identity.
- Existing parity JS suite: **9 passed**.
- Independent Rust fixture: **1 passed, 2 failed**, detailed below. The passing
  case checks actual growth reserve/battery debits and construction heat.
- Independent parity error-path suite: **0 passed, 2 failed**, detailed below.
- Published patch reverse-applies cleanly against the frozen archive. The active
  worker tree has subsequent edits, so reverse-applicability there is not claimed.
- Re-running the read-only pilot reducer produces an object exactly equal to
  the committed parity JSON. Both pinned binaries match its expected SHA
  `90d30b8b1b7f2f7e34a8bcc25d5785144b51f84ed24b80b317d4234c15e82a4f`.
- Independently validated all **24** pilot schema-12 snapshot envelopes and
  compared the **12 reference payloads byte-for-byte** with retained `512ee52`
  counterparts, beyond the reducer's hash comparison. All **12 arms** have
  exactly 720 census rows, ticks 144200 through 288000 at cadence 200, complete
  summaries and reported passing numerical audits. This does not independently
  reconstruct every numerical transaction from censuses.

## Production assessment

At frozen `hunter.rs:452–508`, semantic version 5 explicitly retains fixed
charge80 oxidation and changes only the size-aware growth permission helper.
At `world.rs:1768–1834`, authoritative membership selects the helper at the
existing post-oxidation site; ordinary organisms and versions 3/4 return the
original expression. The strict `R > gate` remains a precondition, not a protected
reserve floor. Rate, remaining structure, whole reserve and battery caps,
mutation order, reserve/battery debits, and `cost + e_r * grown` heat are unchanged.
There are no additional RNG calls, imports, gifts, sensing changes or cap changes.

The harness recipe retains .80/.90 seek/perch, fixed .80 charging and the same
opening inventory. Version 5 is the serialized selector; no new persistent field
or schema layout is introduced, and the default constructor still writes v3.
Mid-growth restart is tested against an uninterrupted world. The old supported
version set `[3,4]` rejects v5 by source inspection; the new suite's unsupported-v6
test is a stand-in, **not an executed genuine-old-reader/v5 test**.

## Corrections before treating the new ledger as completed evidence

1. **The final growth increment is missing from the ledger's boundary metadata.**
   Frozen `flow.rs:510–525` updates `max_structure` and `first_adult_tick` only at
   the pre-growth observation. `record_growth:559–620` does not advance them.
   A synthetic member at S=1.99998, R=3, E=3.5 reaches S=2 on tick 1, with a
   correctly paid remaining-structure-capped increment. Its ledger nevertheless
   reports `first_adult_tick=None` and `max_structure=1.99998`. A following tick
   notices adulthood late; a horizon or death can omit it entirely. Preserve
   pre-growth size/threshold for the first-growth record, but update completed
   size/adulthood at the actual mutation boundary. Do not move biology to repair
   observation. The [independent Rust fixture](assets/astra-hunter-size-gate-boundary-2026-09-13.rs)
   preserves both failures and the passing payment assertion. Install unchanged
   as `crates/cubarium-core/tests/astra_size_gate_review.rs` in the isolated source
   and run `cargo test -p cubarium-core --test astra_size_gate_review`.

2. **Two narrow parity error paths claim identity without matching artifacts.**
   `compareSnapshot({error:'CRC failed'}, {error:'CRC failed'})` currently reports
   `identical_payload=true`, because all compared fields are absent on both
   objects. `divergence([tick0,tick200], [tick0])` reports projected identity
   because it compares only the common prefix. Refuse decoder-error/missing
   snapshot metadata and unequal stream lengths. Preserved in
   [independent JS regressions](../../scripts/astra-hunter-size-gate-review.test.mjs),
   run with `node scripts/astra-hunter-size-gate-review.test.mjs`.
   The actual pilot envelopes and stream lengths were independently checked above
   and are valid: these are future false-certification paths, not an allegation
   that the recorded pilot is corrupt. The parity reducer remains a comparator,
   not a substitute for core audits or mutation-site instrumentation.

## What the pilot does and does not establish

All six unchanged-reference arms reproduce retained payloads, events and census
streams. Candidate untouched/budget/attack-disabled arms retain identical events
and censuses after removing exactly `hunter_state.profile.version`. Hunting arms
first differ in the **sampled census** at tick 170600, child `17:3`; this is not
the mutation tick. The three candidate children reach sampled maxima near
S=.8485, versus .8 reference children. No child reaches adulthood.

| Seed-1 arm | Captures reference → candidate | Paid offspring | Closing hunters |
| --- | --- | --- | --- |
| specialist_on | 114 → 49 | 3 → 1 | 1 → 0 |
| facultative_on | 114 → 90 | 3 → 2 | 1 → 0 |

Both candidate founder lineages become extinct; both reference founders survive
the horizon. These are retained adverse observations, not a selection rule.
Lower capture pressure also means that “every other outcome moved the wrong way”
is too broad: prey closing counts increase (93→99 specialist, 92→99 facultative).

S=.8199 at the first sample is compatible with 199 rate-capped increments after
birth, but it is not a direct record of their timing, payment or oxidation. The
proposal's .8485 arithmetic was explicitly illustrative. The handoff's statement
that the stopping point is “endowment, not the gate being shut” is unsupported:
depleted endowment and the rising gate jointly define that illustration. The
new ledger must show actual gate observations, growth debits/heat, intake and
oxidation before claiming which constraint closed in this trajectory.

## Concrete next action

Finish the already-scoped ledger wiring, fix the mutation-boundary metadata and
two comparator refusal paths, freeze/re-pin, and replay **seed 1 only** to confirm
unchanged payload/event/census outcomes plus complete member-ledger identities
and fixed-tolerance payment reconciliation. This is the existing task, not a new
preliminary tooling project. Retain original pilot artifacts and their limits.

Then run the proposed **unchanged** full 12-seed × six-arm reference/candidate
comparison, including no-child, failed and extinct lineages. It can determine
whether opening the gate produces paid growth across opportunities and whether
its stock-allocation cost is generally harmful, neutral or occasionally permits
maturation. A complete unsuccessful cohort is useful evidence. Do not lower
other thresholds, add resources, select favorable seeds or call partial growth
sustainable recruitment. Read the full paired outcomes before deciding on longer
horizons or another mechanism; numerical passage alone establishes neither.
