---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Independent schema 9 accounting correction review

Reviewed core commit `b47eacc` against
`accounting-compensation-handoff-2026-09-13.md`, plus root's corrected gate in
`crates/cubarium/examples/care_compare.rs` (committed as `57408c4` during this
review). Lore
retrieval preceded direct source/diff inspection. No core, harness, fixtures or
live state were edited by this reviewer.

Disposition: **no blocking correctness defect found in this bounded review**.
This clears the source-review gate, not the outstanding twelve-hour numerical
reruns, ecological balance or deployment gate.

## Compatibility and persistence

- `accounting::accumulate` computes the same `next = old_raw + amount` before
  booking the signed Neumaier error. An exhaustive raw-counter reference sweep
  finds the two production update sites in `World::step`: the existing heat
  closure and field-light admission, at the same locations with the same amounts.
  The transient counter additions remain unchanged and no biology/RNG path reads
  the correction back into ecological decisions.
- `WorldState.energy_correction` is appended after care. The explicit schema 8
  mirror retains the pre-correction field order and nested types; schema 8 and 7
  migrations initialize corrections to zero, without reinterpreting historical
  raw totals. Schema 9 decode, state hash and snapshot all include the correction.
  The legacy ecology projection still drops the appended fields.
- Genuine schema 8 fixtures were committed separately in `3f2900b`; the new test
  compares the complete projected payload to the pre-correction binary's next
  600 ticks, not a reconstructed expected state or a selection of convenient
  ecological fields. I independently ran and passed that test and the existing
  schema 7 600-tick continuation fixture test.
- Validation accepts ordinary negative corrections, rejects nonfinite corrections
  and nonfinite/negative combined totals, and is called at decode and runtime
  invariant checks. There is no silent clamping or reset. The restart test really
  contains nonzero corrections and a shower with 45 samples already delivered;
  reloaded and uninterrupted full hashes agree at every subsequent checked tick.

## Readout, telemetry and the new harness gate

`Ledger::since` differences raw and correction components separately, as required;
core step audits and the harness retain opening `EnergyLedgers` readings rather
than subtracting two rounded corrected cumulative totals. The core cumulative
tests use the corrected representation. `World::telemetry` and its transient
counter resets are unchanged; correction storage is in `WorldState`, not in the
resettable observer.

The harness retains the original raw `max_absolute_drift.energy`, raw closing
residual and `legacy_audit_passed`. Its new primary gate requires original-limit
material/water, persisted corrected energy, independent windowed energy and the
immediate receipt-boundary energy check together. Limits remain
`1e-8 * max(opening_inventory, 1)`, never cumulative-input-scaled. Corrected
energy is measured each tick, including care-ledger deltas; the independent
measurement instead uses short-window flows and actual receipt amounts. Output
clearly labels the new basis, and a failed primary gate still exits unsuccessfully
after printing the evidence. The five harness tests pass, including the test that
rejects any failing component under the same unchanged limit.

One precision boundary is worth retaining in future API usage, not a blocker for
this slice: `EnergyLedgers::net_since` subtracts two individually rounded `f64`
interval flows. It is not an exact multi-component expansion for arbitrarily large
nearly cancelling flows. For example, from zero, light `{raw: 1e16, correction: 1}`
and heat `{raw: 1e16, correction: 0}` net to zero in that helper even though their
represented net is one. This conforms to the selected helper contract and is not
the measured twelve-hour issue: current interval totals near `6e4` have rounding
granularity many orders below the roughly `2e-5` inventory gate. Do not describe
the accessor as universally exact or use this limitation to relax that gate.

## Independently executed checks

All commands exited 0:

- `cargo test -p cubarium-core --offline --test energy_correction`: 7 passed.
- `cargo test -p cubarium-core --offline accounting::tests`: 7 matching unit
  tests passed; other integration suites were filtered out, not rerun by this
  command.
- `cargo test -p cubarium-core --offline --test care zero_care_reproduces_the_pre_change_binarys_next_600_ticks`:
  1 passed.
- `cargo test -p cubarium --offline --example care_compare`: 5 passed, including
  observer-cadence identity and the corrected acceptance gate.

The prior legacy failures remain historical failures; migrations cannot recover
their lost bits. Root's frozen-binary twelve-hour repeats must show both persisted
corrected and independent residuals passing the original limits, while legacy
projections/ecology remain unchanged, before closing the long-run accounting
issue. Nothing in these checks validates hunters or prey recovery.
