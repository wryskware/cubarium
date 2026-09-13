---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Ambient rainfall harness: bounded independent review

Scope: Opus commit `ad8c7a7`, the [isolated support proposal](astra-ambient-support-experiment-proposal-2026-09-13.md), actual water accounting, and narrowly scoped harness corrections below. Lore/Graft context and canon were consulted. No production ecology, configuration default, renderer, live process, or canonical decision changed.

Disposition: ready to **collect** the frozen two-hour, twelve-seed/six-arm experiment with the corrections accompanying this review. This is not evidence for changing natural rainfall, unattended viability, or long-horizon numerical success. No full-horizon run was started here.

## Concrete defects corrected

1. `ambient_compare::main` originally returned success even when its retained summary contained a technical or numerical failure. A finite out-of-limit residual did not fail `step`, and only changed a JSON flag at finalization. The driver now writes the complete retained summary first, requires all twelve seeds and all six arms to be technically complete and numerically passing, and then returns an error on failure. Raw legacy energy drift remains separately visible; no tolerance or arithmetic changed. A non-200-tick diagnostic cadence cannot claim the complete preregistered experiment at the top level.
2. An arm finalization error previously escaped through `?`, discarding the remaining prescribed arms/seeds from collection. `finish_retained` now keeps the actual world tick/population, original step error, finalization error, and explicit incomplete/untrusted-observer markers. Seed-level initialization/output errors are retained at the outer loop so later seeds still run. If even the final aggregate cannot be written (for example persistent storage exhaustion), the command necessarily fails; it does not claim durable complete results. The regression injects an exclusive-write refusal, **not** real ENOSPC, and verifies the existing file is untouched.
3. The extracted audit helpers removed `LifeEvent`/`OrganismId` imports still used by `care_compare` tests. Explicit example testing reproduced three compiler errors. Restored only `cfg(test)` imports. A successful ordinary workspace test invocation does not establish that example test targets compile.

New completion fixtures refuse failed audits, incomplete arms, missing arms/seeds, and an explicit seed failure. Finalization fixtures preserve actual progress and both errors without overwriting output. These changes are confined to the two example entry points; the shared accounting implementation is unchanged.

## Verified contract and limits

- Six arms are exactly untouched natural rain versus `opening_rate * 0.9`, independently crossed with no input/Standard 1000/Generous 1500 rain. `arm_state` clones before the one candidate field assignment. The reference performs no percentage round trip. Tests rewind that one field and compare the entire state; no stocks, RNG, weather, IDs, capacity, other configuration, hunters, Feed, or Clean are changed.
- Each seed has one common material/chemical-energy/water inventory and fixed `1e-8 * max(opening_inventory, 1)` limits, read before any candidate configuration. Material includes offspring escrow; energy includes reserve and paid escrow. Persisted compensated energy uses component-wise interval ledgers. Independent energy uses exactly one telemetry reset per window plus current transient counters; manual rain imports no material or chemical energy. Water counts manual rain once inside total rain, with `care.rain_depth_in` only attribution. Natural rainfall is the remainder, not an additional booked source.
- Core `water::step` computes natural input from the unchanged weather excess and the chosen rain rate; manual input joins it before flow/evaporation. Identical weather/rate imply identical natural **driver**, but subtracting cumulative attribution from cumulative rain is not bit-exact across doses. The separation test retains a much tighter noise bound than the ordinary water audit. It does not pretend accumulated read-back is exact.
- Rain is scheduled before stepping boundary 60, then every 2400 ticks, rotating the three fixed interior/seam/rim targets. Every attempt/outcome/reason is streamed. The generous restart fixture snapshots 40 ticks into a 120-tick shower at 90% support and compares resumed state hashes through completion. It tests world/shower persistence, not experiment-output resume: the harness deliberately refuses to resume an existing output directory.
- All twelve fixed schema-9 tick-144000 inputs are checksummed, decoded and checked against manifest seed/census/ecology identity, and required to contain no care/hunters. Original snapshots are read-only. Per-arm files and output directories use exclusive creation.
- Samples/receipts stream to disk; ancestry retains only living IDs. Three frozen graph neighborhoods include seam transport and use constant-sized accumulators. Water stock and water-depth-tick integrals remain distinct; flooded/drowned cell-time is an exposure measure, not proof of biological damage. At 72 hours each arm emits 25,920 ordinary 200-tick samples and 2,160 scheduled receipts in cared arms. Output size grows with duration, not retained in-memory history. Seventy-two-hour runtime/audit behavior has **not** been measured here.

## Existing frozen smoke independently checked

`captures/ambient-rain-smoke-ad8c7a7` contains a complete aggregate and all twelve per-seed results. `/tmp/ambient-smoke.log` reaches `seed 12/12 complete` and the final output path. No replacement smoke process was launched. These are completed artifacts, not an independently recovered native process exit code (the original command's zero exit alone would not have certified its audits).

Build `0.1.0+ad8c7a7`; retained executable SHA-256:
`7201588899d2fe99a8c98b1fa75116924389470f3261e5fbb3fbb881e2a62a55`.

Independently parsed all 72 arms, 864 samples and 48 Applied rain receipts. Every closing snapshot SHA matched; arm/seed/aggregate duplicate summaries agreed; samples were sequential at 200 ticks through 2400, final sample hashes matched closing hashes, census-by-form summed to population, all common PRE inventories agreed, and all configurations differed only by the declared natural rain field. Every arm reported technical completion and passing strict audits; every smoke measurement-completion flag remained false. The smoke exercises the first interior shower, not every target in the rotation.

Maximum residuals over these actual artifacts:

| Quantity | Maximum |
| --- | ---: |
| Material | `7.958078640513122e-12` |
| Water | `4.311573320592288e-11` |
| Persisted corrected energy | `4.860112312599085e-12` |
| Independent windowed energy | `1.4864554032101296e-11` |

Maximum attributed-natural difference between doses at the same support was `9.094947017729282e-12`; delivered manual water at the same dose was identical across support levels. These short smoke results do not establish a two-hour or 72-hour conservation pass.

## Focused verification after corrections

`CARGO_TARGET_DIR=captures/build-cache/ambient-review cargo test -p cubarium --example ambient_compare --example care_compare`: **16 ambient + 9 care tests passed**, exit 0. This includes real-cohort refusal tests (the twelve source snapshots were present), no-input observation identity, exact field isolation, independent factors, refusal retention, strict budget faults, and mid-shower restart.

Built both examples and compared new `care_compare` stdout byte-for-byte against the real frozen `4d10351` executable: seed 1/default recipe/2400 ticks (13,505 bytes), and seed 3/Generous isolated Rain/start 60/target 2/local cadence 200/2400 ticks (244,332 bytes). Both invocations exited 0 and matched completely. Also reconfirmed the three existing `/tmp/ambient-{baseline-frozen,baseline-head,after-refactor}.json` artifacts are byte-identical. Directly invoking the updated ambient binary against the existing smoke output returned exit 1 with `File exists`; no files were overwritten.

The two-hour collection should freeze the committed corrected harness and retain all 72 arms, including empty or unfavorable worlds. Its technical gate must pass before interpreting biological contrasts. A successful two-hour run still does not authorize a production support reduction or turn the controlled two-minute rain exposure into a human-care obligation.
