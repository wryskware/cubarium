---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Implemented: persisted energy precision in cubarium-core (schema 9)

Implementation record for
`design/7_Research/accounting-compensation-handoff-2026-09-13.md`, the bounded correction
proposed by `astra-long-run-energy-diagnostic-2026-09-13.md` and motivated by
`care-twelve-hour-results-2026-09-13.md`. This is evidence, not a decision: nothing here
supersedes canon, and no live process or state was touched.

Scope actually edited: `crates/cubarium-core/` and this file. No hunter, art, host,
renderer, live state or comparison-harness source was modified.

## What the correction is

Every existing raw addition to `light_in_total` and `heat_out_total` still happens, in the
same place, in the same order, producing the same `f64`. Alongside each one the bits that
addition dropped are accumulated into a persisted signed Neumaier component:

```text
next = s + amount
c   += |s| >= |amount| ? (s - next) + amount : (amount - next) + s
raw  = next
```

`crates/cubarium-core/src/accounting.rs` owns that arithmetic (`accounting::accumulate`) and
the read side. `WorldState` gains exactly one appended field,
`energy_correction: EnergyCorrection { light_in, heat_out }`. The corrected cumulative total
is `raw + correction`; a precise interval flow is
`(raw_end − raw_start) + (correction_end − correction_start)`, never the difference of two
corrected totals.

The raw counters keep their names, their values and their place on the wire. They are now
documented as uncompensated diagnostics and legacy wire values — not as accurate cumulative
totals — everywhere they are exposed.

### Update sites, audited exhaustively

`light_in_total` has exactly one addition site in the crate (`world.rs`, tick step 3, the
field-reaction ledger). `heat_out_total` has exactly one, the `heat` closure in `World::step`,
through which all eleven heat payment sites flow (field reactions, maintenance, the three
oxidation paths, escrow release-to-energy, growth, build, both death paths, and the escrow
that becomes structure at birth). Both now go through
`accounting::accumulate`. No other module, test or crate adds to either counter; the only
external writers in the workspace write `external_material_in` and `rain_in_total`, which are
out of scope (below).

`TickCounters` / `Telemetry` keep their existing arithmetic and their reset-on-sample
semantics untouched: they are the observer's short-window flows, and the diagnostic note
depends on them staying exactly what they were.

Care accounting is unchanged: the six care ledgers, the allowance, the shower envelope and
`apply_care` are byte-for-byte as before.

## Persistence

- `SCHEMA_VERSION` is now **9**. Schema 9 is schema 8 plus two appended `f64` (16 bytes).
- `snapshot/v8.rs` freezes `WorldStateV8`, the field list as of `c60241f`, with `project()`
  and a migration that opens the corrections at **zero**.
- `snapshot/v7.rs` is unchanged apart from also opening the corrections at zero.
- `decode_snapshot` accepts 9, 8 and 7 and reports which it read; 6 and older are still
  refused outright.
- `ecology_hash` still hashes the schema 7 projection, so care/no-care comparisons and
  `care_replay` are unaffected. `state_hash` covers the corrections and therefore changes —
  that is the intended consequence of correcting an audited counter, not an ecological change.

Migrated corrections are zero. **No historical rounding repair is claimed**: the low-order
bits a live world's raw totals have already lost are not recoverable from a snapshot.
Compensation begins at the migration and covers flow booked after it.

Validation: corrections must be finite, and each *combined* total must be finite and
nonnegative. An unusable accounting state fails `WorldState::validate` (and therefore the
decode) and fails `check_invariants`; nothing is clamped, zeroed or repaired silently.

## API for integration

```rust
// read side, on WorldState (and delegated from World)
state.energy_ledgers() -> EnergyLedgers { light_in: Ledger, heat_out: Ledger }
state.light_in_corrected() / heat_out_corrected() / net_energy_in_corrected() -> f64

// Ledger { raw, correction }
ledger.total()          // raw + correction
ledger.since(opening)   // (raw_end-raw_start) + (correction_end-correction_start)

// EnergyLedgers
ledgers.net()                 // corrected light_in − heat_out
ledgers.net_since(opening)    // each ledger differenced precisely, then subtracted

// write side, for any future compensated counter
accounting::accumulate(&mut raw, &mut correction, amount)
```

Re-exported from the crate root: `EnergyCorrection`, `EnergyLedgers`, `Ledger`, `SCHEMA_V8`,
`WorldStateV8`.

The in-step debug energy audit and the crate's cumulative energy audits now read the
corrected representation. Single-tick transfer checks in unrelated behavioural tests still
difference the raw counters; over one tick the two agree to well under a ulp.

## Evidence

Commands, run offline in this working tree:

```text
cargo test -p cubarium-core --offline
  lib 127 passed / 0 failed / 2 ignored
  accounting 5, care 13, controller_rules 6, determinism 6, energy_correction 7,
  population 2, redesign_rules 32, settlement 2, snapshot_hardening 8 — all passed
  total 208 passed, 0 failed
cargo clippy -p cubarium-core --offline --all-targets   # clean, no warnings
cargo check --workspace --all-targets --offline         # clean: no host edit was needed
```

`cargo fmt` is *not* run: this repository does not conform to default rustfmt (it reports
hundreds of diffs across every pre-existing file in the crate), so the new code follows the
surrounding style instead.

### Genuine fixture continuations

- Schema 7: the existing `zero_care_reproduces_the_pre_change_binarys_next_600_ticks`
  (`tests/care.rs`, fixtures `live-v7-55200*.cubw`) is untouched and still passes — the
  schema 7 projection of 600 stepped ticks is byte-identical to the pre-change binary's.
- Schema 8: `zero_corrections_reproduce_the_pre_correction_binarys_next_600_ticks`
  (`tests/energy_correction.rs`) loads root's genuine `live-v8-172800.cubw` (tick 172,800,
  admitted care sequence 5), steps 600 ticks and re-encodes the schema 8 projection: it is
  **byte-identical** to `live-v8-172800-plus600.cubw`, care ledgers included. Fields,
  organisms, weather, config, both raw energy counters, both water counters, birth/death
  counters, the care state and `ecology_hash` are additionally asserted individually so a
  failure would name what moved.

### Measured behaviour of the correction

From that live world, 600 ticks after tick 172,800 (test output):

```text
light correction -1.706335073237142e-11
heat  correction -6.871147803880364e-9
corrected net 4.93234308725745763e0   raw net 4.93234308040337055e0
```

Scaling that heat correction by the 1,440 windows of a twelve-hour run gives ~1e-5, the same
order as the 2.0344e-5 persisted-minus-windowed heat discrepancy the diagnostic measured on
its own seed and schedule. That is corroboration, not a repeat of that run.

From a fresh default world, 3,000 ticks, against an independent per-tick windowed Neumaier
sum of the transient counters (test output):

```text
windowed light 2.23093769210195205e2   heat 3.92911554315269257e2
corrected − windowed:  light  0e0            heat -5.684341886080802e-14
raw       − windowed:  light -3.979039320256561e-13  heat  1.5921841622912325e-10
corrections:           light  3.8410941094468853e-13 heat -1.5929201922030946e-10
```

The residual `5.7e-14` is the observer's own per-tick rounding; the raw heat counter is three
orders of magnitude further out after only 3,000 ticks. The 2,000-tick audit in
`tests/accounting.rs` reports the same shape: corrected `1.42e-14` from the summed samples,
raw `-1.35e-11`.

### Other tests added (`tests/energy_correction.rs`, `src/accounting.rs`)

- `accumulate` against exact references: `2^20` additions of `2^-60` to `1.0` recovered
  exactly (`1 + 2^-40`) while the raw counter stays at `1.0`; and 20,000 **signed** amounts
  below the raw ulp matched bit-for-bit against an `i128` reference sum.
- The raw counter equals the naive sum after every addition, including mixed magnitudes and
  both zeros and subnormal-scale operands.
- Non-finite amounts poison the state and are caught by validation rather than absorbed.
- Decode hardening: NaN/±∞ corrections on either ledger, a correction that drags a corrected
  total negative, and an overflowing total are each refused with the failing ledger's name;
  signed corrections of both signs round-trip.
- Restart: a world checkpointed mid-shower with partially accumulated corrections on both
  ledgers reloads to the same full-state hash, stays hash-identical to the uninterrupted run
  for the remaining 180 ticks, and ends with bit-identical corrected ledgers.
- Migration from schema 7 and schema 8 opens at zero corrections and is compensated from
  there; `ecology_hash` is unmoved by any correction.

## What this does not establish

- It does not repair any existing live world's historical totals, and does not make a
  migrated world's opening raw totals accurate.
- It does not repeat the twelve-hour comparison. Root's harness integration and a repeat of
  the failed seed/schedule cases under the unchanged threshold are still required before the
  long-run accounting issue can be closed. Other failed seeds may have other causes.
- No live deployment was performed and no live state was touched.

## Remaining integration needs (root)

1. **Harness read side — already fits.** Root's working-tree
   `crates/cubarium/examples/care_compare.rs` (which I did not edit; root owns it) already
   calls `initial.energy_ledgers()` and `s.energy_ledgers().net_since(opening)` for its
   corrected residual, alongside the raw-counter residual it keeps reporting separately.
   `cargo check --workspace --all-targets --offline` is clean, so that integration compiles
   against this implementation as written. Nothing in the host needed changing.
2. **Snapshot compatibility direction.** New snapshots are schema 9. Schema 9 reads 8 and 7,
   but the frozen `c60241f` binaries cannot read schema 9 — once a live world is resumed on
   this build its checkpoints are no longer loadable by the pre-correction tooling. Worth a
   copy of the live state before the first resume.
3. **Telemetry.** `Telemetry` deliberately gained no field, so the host's JSON output is
   unchanged. If the twelve-hour reports want corrected cumulative totals in the sample
   stream, that is a one-line core addition plus a host reader change — say the word.
4. **The same defect elsewhere, not fixed here.** `rain_in_total`, `evap_out_total` and
   `external_material_in` accumulate the same way and are read by `water_residual` and
   `mass_residual`. `external_material_in` is written only at world creation — the tick never
   adds to it — so it is not a concern; the two water counters take one addition each per tick
   (against up to eleven payment sites per tick for heat), and root's evidence has not shown
   their audit failing. Extending the same
   compensation to them would be a second appended field and another schema bump; the handoff
   bounds this work to the energy ledgers, so it was not done. It should be folded into
   whichever schema change comes next if wanted.
