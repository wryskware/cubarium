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

## Completed fixed-cohort screen

The tool was frozen at `4d10351` in `/tmp/cubarium-care-response-4d10351`, where
all nine example tests and the release build completed successfully. Runner
`scripts/run-care-response.mjs` (`a348b90`) then completed **108 comparisons**:
all twelve prescribed mature openings × three fixed targets × three isolated
Standard actions, each against its own untreated copy. The process exited zero;
all artifact checks passed. Raw output and executable/input/script hashes remain
in `captures/care-response-single-pulse-4d10351/{manifest,summary}.json` and its
per-case JSON files. Existing outputs are refused, not overwritten. Log:
`/tmp/cubarium-care-response-single-pulse.log`.

Each comparison has 30 seconds before the pulse and 120 seconds after it. All
nine untreated arm reports for each seed are exactly equal, and both arms are
identical through the pre-pulse boundary. This is a short response screen, not
a new care mechanism, a long-run balance test, or a live intervention.

Root's independent target-region reduction subtracts cumulative counters at
elapsed 600 from those at 3000; it does not include the pre-input interval:

| Action | More / less / unchanged actual-fed member-ticks, cared versus untreated |
| --- | --- |
| Feed | 27 / 7 / 2 of 36 fixed seed-target pairs |
| Rain | 14 / 16 / 6 |
| Clean | 9 / 25 / 2 |

Seven of the 36 pre-pulse local cohorts are empty, retained for every action.
These signs describe local organism-time, not causal intake of manually deposited
food or a motion measurement. Cleanup's frequent feeding reduction is consistent
with exporting edible substrate; rain's mixed result is not a growth benefit.

The quiet-habit gap is now measured rather than merely hypothetical: summed over
the 36 target regions, untreated post-pulse Resting is only **6 member-ticks**,
versus Feed 13, Rain 6 and Clean 6. This overlapping-region count is not unique
world-population time. At the pre-pulse boundary the 69 region-member records
(not necessarily distinct individuals across regions) contain 42 Feeding and
27 Seeking, none Resting; hunger-memory median is approximately 0.829. Do not
manufacture satiated rest in presentation or claim existing rest hysteresis
commonly produces long quiet bouts in these mature worlds.

The independent [complete per-seed/fixed-ID reduction](astra-care-response-results-2026-09-13.md)
is now checked in, with a read-only reproducer; root reran it successfully.
Next: a presentation-only meal-onset/settling prototype to compare whether actual
intake becomes easier to recognize, not an assumption that the current art fails.
Preserve ordinary ecology while evaluating that prototype. Longer-lived quiet
habits need separate resource/drive evidence, not simply relabeling Feeding as
Resting or forcing a crowd response after every input.
