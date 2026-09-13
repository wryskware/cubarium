---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Distinguishing long-run ledger rounding from transfer drift

Root reports strict 12-hour care-comparison energy residuals around 2e-5. The
opening-inventory-scaled threshold remains unchanged and those runs have not
passed that audit. This note proposes an independent observation method from
the current public API; it does not diagnose the observed failures as harmless.
No core or comparison-harness source was edited by this reviewer.

## What the current source establishes

`World::step`'s heat closure adds each payment separately to both
`self.counters.heat_out` and `state.heat_out_total`. Many small maintenance,
feeding, oxidation, build and death payments therefore accumulate directly into
an increasingly large persisted double. Field light is accumulated per tick and
then added to both the transient and persisted light counters.

The returned `TickCounters` are accumulated since the last `telemetry()` call,
despite their name. `care_compare` currently calls telemetry only after its loop,
so treating each returned value as that tick's flow would double-count history.
Likewise subtracting adjacent persisted totals cannot recover increments already
rounded away when originally added to those totals.

`World::telemetry()` resets transient counters and diagnostic neighbour statistics,
but not `WorldState`, field values, organisms, or RNG state. Its reset provides a
second, shorter-scale measurement of the same source/sink increments without
changing the ecological implementation. Verify that claim with equal final full
state hashes across two observer cadences in the actual diagnostic run.

## Narrow harness measurement

1. Start from the same exact initial state and care schedule. Clear transient
   counters once with telemetry before the first step. Retain the original strict
   cumulative audit as-is, including its pass/fail result.
2. Reset transient counters every 200 ticks initially. Copy the interval's
   `light_in` and `heat_out` once (from telemetry or the last step's counters),
   and accumulate those interval totals using independent Neumaier/Kahan sums.
   Never add every accumulated `TickCounters` value to the external sum.
3. At any intermediate tick, the external total is closed-interval sums plus
   the current unreset interval counters. On a reset, move that interval into the
   closed sums exactly once. Include the final partial interval.
4. Sum each care receipt's actual `energy_in` and `energy_out` with compensation,
   separately from cumulative care ledgers. Around each `apply_care`, measure
   stored energy before/after immediately and retain its boundary-only residual.
   Rain has no chemical-energy source term in this model.
5. Evaluate a second signed cumulative residual using those measured short-window
   flows, with precisely the same opening energy and acceptance threshold.
   Keep its result separate from the original persisted-counter residual.

For stored-energy change `dE`, persistent counter deltas `dL,dH,dF,dC`, and
short-window/receipt sums `Lw,Hw,Fw,Cw`:

```text
Rp = dE - dL + dH - dF + dC       // existing strict audit
Rw = dE - Lw + Hw - Fw + Cw       // independent observation
Rp - Rw = (Lw-dL) + (dH-Hw) + (Fw-dF) + (dC-Cw)
```

Report those four signed ledger discrepancies at the same ticks as both
residuals. Do not subtract separate worst-absolute peaks: they can occur at
different times with different signs. Record the tick and sign of each peak,
the first threshold-crossing tick, interval width, seed and care mode.

The stored-energy reader also has summation-order rounding: the harness currently
sums producer/fruit first and then multiplies their energy densities, whereas the
core's debug helper multiplies each cell first. At defaults producer density2 is
an exact binary scaling, but fruit density3 is not. Measure a compensated sum of
per-cell/per-organism energy terms alongside the existing reader. This can bound
stock-readout error; do not alter the original audit to hide a difference.

## Interpreting the evidence

If Rw remains comfortably below the unchanged threshold while Rp fails, and the
signed ledger-discrepancy equation explains the difference to readout rounding,
that is evidence for persisted-counter accumulation error. Confirm with shorter
intervals (20 ticks, then one tick only if needed) and unchanged world-state hashes.
The reported strict audit still fails until a separately reviewed correction is
implemented; a better diagnostic sum is not retroactive validation of persisted
accounting.

If Rw also drifts comparably, isolate step transfers. Within each short interval,
compute step flow as the difference of adjacent **transient** counters; compare
that against immediate stored-energy change, excluding separately measured care
boundaries. Save the largest signed step errors with births, deaths and care
receipts. Short-interval differencing has its own smaller rounding error, so use
one-tick resets around a suspicious reproduction/death/feeding segment before
attributing the error to a transfer rule.

Current source makes long-counter rounding plausible but does not establish it
as the cause of root's measured failures. This comparison should precede paid
hunter transfer implementation; no tolerance relaxation is proposed.

## Measured disposition: seed 1, repeated care, twelve hours

Read the actual reports in
`captures/care-longitudinal-2026-09-13/seed1-repeated-window200.json` and
`seed1-repeated-window20.json`. Both run 864,000 ticks from seed 1 with care every
2,400 ticks. The no-care world's full state hash is `2601251707681483130` in both
reports; the cared world's hash is `5399110610278733365` in both. Changing observer
cadence therefore did not change either world state.

The cared persisted-counter peak is `2.0506936266428966e-5`, above the unchanged
limit `1.9563930834586353e-5`: its original audit still fails. Independent windowed
peaks are `6.281197784119286e-10` at 200 ticks and `2.8410340746631846e-10` at
20 ticks. Both report a maximum care-boundary error of `4.547473508864641e-13`.

At the final tick, cared persisted energy residual is `2.034550846019556e-5`.
The persistent-minus-windowed heat discrepancy is `2.0344043150544167e-5` in
the 200-tick report and `2.034491626545787e-5` in the 20-tick report. Light
discrepancy is only `8.585629984736443e-10`; feed discrepancy is zero and cleanup
discrepancy is `4.547473508864641e-13`. The signed discrepancy identity above
accounts for the failed residual. The no-care arm also has a roughly `1.68337e-5`
heat-counter discrepancy, although its peak remains inside this run's limit.

Conclusion: these matched reports establish persisted heat-counter accumulation
rounding as the dominant cause of this seed/schedule's failed energy audit. They
do not establish every other failed seed's cause or retroactively pass persisted
accounting. In this case the independent stock/flow observations provide strong
evidence against a care-transfer leak at the measured scale.

The narrow correction is compensated accumulation of the persisted energy source
and sink ledgers, retaining their current units and threshold. Persist the
compensation state with an explicit snapshot migration so restart does not discard
it. Coordinate that extension with the next schema change rather than issuing
competing schema numbers for accounting and hunters. Migrate old compensation to
zero; the existing cumulative totals are historical observations and their lost
low-order bits cannot be recovered from a snapshot alone.

Verify from a fixed opening snapshot that corrected ledger accumulation tracks
the independently windowed totals under the original threshold, and test resumed
versus uninterrupted compensated state exactly. Compare fields, organisms,
weather and care transfers to the pre-correction run; full state hashes will
necessarily change when the audited counters change, so do not mistake that
intended correction for altered ecological behavior. Repeat the other failed
seed/schedule cases before closing the long-run accounting issue. No correction
was implemented by this review.
