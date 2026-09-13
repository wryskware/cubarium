---
design_status: exploration
last_reviewed: 2026-09-13
decision_refs: []
---

# Isolated care pulses and bounded local activity observation

The copied-world `care_compare` tool now supports `--care-kind cycle|feed|rain|clean`,
`--dose-permille`, `--care-start`, `--target-index` and optional `--local-every`.
This implements the first measurement part of
[Astra's care-response recommendation](astra-local-care-response-slice-2026-09-13.md),
not a new biological response or a claim that all care is beneficial.

Default behavior stays the original mixed cycle: Feed at zero, Rain at 60 ticks,
Clean at 300, then repeat and rotate the original interior/seam/rim targets.
An isolated kind instead acts at the common start boundary and once per period;
choose a period longer than the horizon for a single pulse. A delayed start gives
both arms an identical pre-input interval. Amount is the actual core `CareDose`.
The unchanged conservation, independent energy and receipt checks still run.
Hunter-profile openings are now explicitly refused because this tool's energy
inventory does not account for hunter gut stores; use the hunter harness for those.

Example: 30 seconds before one Standard Feed, then two minutes afterwards:

```sh
cargo run --release -p cubarium --example care_compare -- EXACT_OPENING.cubw \
  --ticks 3000 --care-kind feed --care-start 600 --care-every 72000 \
  --dose-permille 1000 --target-index 0 --local-every 20
```

Every run still emits untreated and cared arms. Repeat the same opening/target
for rain and cleanup; the repeated untreated reports should be identical.
Targets are fixed before measurement, not selected for a pleasing response:
Front (32,48), Front (63.5,48), Right (32,63.5).

## What is measured

Three fixed graph-radius-3 regions use the existing surface adjacency, including
seams and the open rim. Every completed tick contributes population, actual-fed,
Resting/Seeking/Feeding-mode, gestating and form member-ticks. Samples report
instant counts, occupied cells, N/P/F/D/De/water and cumulative counters. Counts
remain tickwise even if output cadence is slower; the opening instant is not
included in the cumulative denominator. Output is bounded to 2000 intervals plus
the opening sample; a longer request must explicitly choose a coarser cadence.

Immediately before the **first** pulse, both arms freeze exact local IDs, including
slot and generation, with initial diet, mode, hunger memory, reserve headroom and
sensing radius. Those IDs are then followed anywhere; newcomers and descendants
do not replace losses. This is a fixed-local-ID cohort, not a filtered set of
eligible scavengers. Empty cohorts are retained. Each region's cohort metadata
and living/cumulative counts are reported separately from dynamic local occupancy.

`fed_this_tick` means any actual field intake, not amount eaten or proof that a
manual crumb was consumed. Feeding mode alone is not ingestion. Spatial regions
can overlap and cannot be summed as disjoint populations. Stock differences are
not uptake measurements: reactions, transport, growth and deaths also move stock.
Repeated care after the first pulse does not redefine the fixed cohort.

This first measurement package does **not** yet provide movement-path sums,
rest-bout durations, per-diet intake, food-quality-derived edible D, or flood
cell-time. It does not infer behavior/readability from a receipt or a pooled count.
No live world, controller, resource, RNG, art or persistence behavior was changed.

## Executed checks

`cargo test -p cubarium --example care_compare`: **9 passed, zero failed**.
Tests cover existing audits/ancestry and default repetition, bounded new options,
isolated same-boundary actions and requested dose, seam/rim neighborhood counts,
actual-fed versus feeding-mode counts, fixed IDs after movement/replacement,
duplicate/skipped observer-tick refusal, pre-pulse equality, and complete arm
report equality with local observation disabled (apart from the added local data).

Actual release executables also ran seed 1 for 2400 ticks with default options:
new versus pre-change `e1aa426` from the retained clean worktree. Both entire arm
JSON reports (`baseline` and `cared`) are exactly equal, not merely population or
legacy ecology hashes. Logs/artifacts: `/tmp/cubarium-care-local-tests.log`,
`/tmp/cubarium-care-new-default.json`, `/tmp/cubarium-care-old-default.json`.
That short executable comparison is not multi-seed response evidence.

Next: freeze this committed tool, use all twelve already-prescribed aged openings,
and retain all three fixed targets and all three isolated Standard actions with
30 seconds pre-input / 120 seconds post-input. Compare paired actual-fed and quiet
occupancy as well as water/food stocks, including zero-opportunity and negative
responses. Only then choose the smallest next behavioral/presentation change.
