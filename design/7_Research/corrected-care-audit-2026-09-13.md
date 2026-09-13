---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Care comparison: persisted compensation plus independent verification

Root integrates the `b47eacc` core read API into `care_compare`. The old artifact
executable and every prior raw-ledger failure remain unchanged. This is a new
comparison, not a reinterpretation of those failed commands as passing.

Every tick now reads `energy_ledgers().net_since(opening)`; each ledger differences
its raw and signed correction components before combining them. This avoids
subtracting two large rounded corrected totals. A loaded v7/v8 checkpoint opens
with zero corrections and is audited relative to its opening inventory: the
program does not claim to recover previously lost precision.

The unchanged tolerance is `1e-8 * max(opening_inventory, 1)` per resource. A run
passes only if material, water, persisted corrected energy, independent windowed
energy and immediate care-boundary energy all pass. Nonfinite values fail.
No tolerance grows with gross care inputs or accumulated light/heat.

For compatibility and transparency, `max_absolute_drift.energy` still reports
the raw legacy error, and `legacy_audit_passed` states that old gate's result.
`corrected_energy_audit` names the corrected peak/closing error and result;
`corrected_cumulative_energy_delta` reports its two flows. `audit_basis` explains
the distinction in each JSON arm. Main exit status uses the new conjunctive audit;
it still prints complete reports before returning failure.

Five example tests pass: CLI/schedule bounds, schedule shape, generation-safe
ancestry, repeatability/observer-frequency independence, and an explicit gate
test proving each independent energy check and both other resource checks can
fail the result at the original threshold (including nonfinite inputs).

The core worker reports208 passing tests, v7/v8 genuine continuation equality,
and schema9 restart equality during a shower; root has requested Astra's source
review. Longer corrected comparisons are the next evidence, not yet a passed
gate. No live state or transport is used by this example, and no live rollout
has happened. Schema9 snapshots cannot be read by frozen v8 tooling.
