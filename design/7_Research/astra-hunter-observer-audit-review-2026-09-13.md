---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Hunter experiment accounting observer: bounded review

Reviewed root's new [audit module](../../crates/cubarium/examples/hunter_compare/audit.rs)
against the current core initializer/telemetry contract. Both harness and core
remain in progress. No accounting, core, art or experiment source edited.

## Verified sound

- The saved opening is PRE-import, common across arms. It is not replaced by
  `World::from_state`'s reconstructed mass baseline. The missing-receipt regression
  explicitly demonstrates why this matters.
- Inventory includes field material, organism structure/reserve/escrow and gut
  material. Energy includes producer/fruit chemical energy, detritus energy,
  organism battery and reserve, escrow battery plus `er*(structure+reserve)`,
  and gut energy. Adult structural material itself contributes zero energy.
- The actual founder/control receipt is checked against both inventory deltas
  and the extension's import ledgers at initialization. Control cap heat is
  present in the persisted compensated heat ledger but NOT in current transient
  counters; adding receipt `energy_heat` once to independent observer heat is
  correct. The targeted cap-heat test verifies no double count.
- Corrected energy uses `EnergyLedgers::net_since`, preserving correction deltas
  instead of subtracting rounded corrected totals. The independent energy
  residual uses windowed light/heat plus actual receipt energy. Legacy raw
  energy remains separately reported, not silently substituted as the gate.
- Tolerances stay fixed at `1e-8 * max(common opening inventory,1)`. Equality at
  the bound fails; nonfinite residuals record a failure rather than disappearing
  through a maximum. Resource errors are not excused by later population success.

## Resolved during review: later source growth must match receipts

In the initial draft, `observe` subtracted the CURRENT hunter import-ledger delta from
material, and the receipt-to-ledger comparison runs only in `initialize`.
After a valid initializer, the following synthetic defect would therefore
cancel out of every residual:

```text
fields.d[cell] += 1
hunters.founder_material_in += 1
```

No energy change is necessary: detritus can contain structural material with
zero stored chemical energy. The material residual gains +1 inventory and
subtracts +1 import ledger; both energy checks and water remain unchanged.
But the trial had no second actual receipt. This is a correlated-ledger blind
spot, not a new-baseline reset, and the initial-boundary tests do not cover it.

Root corrected this before final review: throughout the one-initializer arms,
hunter material/energy import deltas must equal the saved actual receipt, and
`external_material_in` must stay unchanged. The new adversarial regression injects
paired +D/+hunter-import and +D/+external-import defects after initialization,
checks returned failure and serialized failed status, then undoes the mutation
and confirms the failure remains sticky. The original ledger-based residual is
retained. I read the correction and reran the tests successfully.

## Report status and observer-window contract

In the initial draft, some `ensure!` exits preceded updating `AuditReport.passed`:
initializer receipt/ledger mismatch, repeated initialization, bad boundary/input,
and care admission. A caller retaining the error can nevertheless serialize a
report whose boolean is still true. This is now corrected: initialize/observe
wrappers retain the first error string and set failed status before returning;
later observation refuses to clear an existing failure. The adversarial test
checks returned error AND report status. A dedicated failure tick is not stored
for every admission error; callers still have the observation's current tick.

`World::step` returns a borrowed `TickCounters`, cumulative since the last
`world.telemetry()` reset, not a `Telemetry` and not necessarily one tick's flow.
The original observer parameter mismatch is now fixed: copy light/heat to scalar
values before borrowing world state, then pass those values to `observe`.
`close_window` adds the reset's returned totals exactly once. Repeated intermediate
observations must not accumulate that still-open window again.

Audit opening also requires transient counters to start at zero. Root confirms
all actual arms construct `World::from_state`, which provides that condition.
The precondition is now documented on `Audit::new`; a caller opening an audit on an already
stepped world must reset telemetry first or subtract its opening window counters.
Otherwise pre-opening flows produce a false failure. This is observer setup,
not authority to reset any persisted resource ledger or ecological baseline.

## Executed evidence

`cargo test -p cubarium --test hunter_observers audit::tests -- --nocapture`
passed **6 tests** on the final reviewed revision, including founder receipt,
baseline-reset trap, cap heat, strict finite/boundary gate, observer-cadence
identity, and the new unreceipted-import/sticky-failure regression. Five tests
had passed before the additional defect was corrected; the second run verifies
the corrected version rather than treating that earlier result as sufficient.

The final inspected care gate also freezes each of the four feed/clean M/Q
ledgers, rather than only their net differences, and requires no admitted care
sequence or active shower. If the harness claims the ENTIRE care state stays
unchanged, also compare `rain_depth_in` and `allowance_used` (or the complete saved
`CareState`); root received that small follow-up. It does not reopen the verified
import and energy corrections.

Core sources checked: [world initializers/telemetry](../../crates/cubarium-core/src/world.rs),
[organism material](../../crates/cubarium-core/src/organism.rs),
[compensated ledgers](../../crates/cubarium-core/src/accounting.rs), and
[hunter inventories](../../crates/cubarium-core/src/hunter.rs).
This review establishes bounded accounting behavior, not predator viability,
prey recovery, or long-run balance.
