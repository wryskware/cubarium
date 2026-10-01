---
design_status: exploration
last_reviewed: 2026-09-29
decision_refs: []
---

# Census length audit: were the 4 h and 6 h runs needed?

The evidence is in `census-length-audit-2026-09-29/findings.md`: 17 census runs or run
families, from 2026-09-20 to 09-28, with a per-run table. Raw per-minute CSVs survive only for
09-20 and 09-21; later rows are read from write-ups and commit messages. This audit made no
new runs.

## Answer

- **Most verdicts were in within the first hour.**
  - 9 of the 14 finished censuses had their headline verdict by 60 min, and 12 by 120 min.
  - Some verdicts were visible at t = 0 (D1, D5) or within 10 min (the WX1d lake drain).
  - Several conclusions were actually drawn from 60-min autopsies (D3–D5, startup, retrain).
- **The long tail caught four things.**
  - The glowcap collapse, on the dead-wood clock (τ 167 min).
  - Confirmation of the glowcap fix, which needed ≥ 240 min.
  - Velvetpad's slow decline.
  - Time-of-day rain shares, which need whole 2-hour days.

  Each of the three declines had an early signal the census doesn't log: the dead-wood
  stock, root-box water, or the growth rate.
- **The long runs didn't deliver what they were for.**
  - No 6 h run reached a steady state: umbrellafrond and litter were still climbing at
    360 min. So "stable at 6 h" was never actually tested.
  - Single-seed end numbers are noise: 802 shredders against 0 across seeds of the same arm.
- **Since lifespans shipped (09-25), animal age matters.**
  - Founders start at age 0 with 96–288 min left (`draw_lifespan_ticks`), so no animal
    dies of age before minute 96.
  - The first offspring generation dies at 250–390 min.
  - So a fresh 1–2 h run cannot show an edit to lifespan (240-min clock) or defence
    (90-min clock).
- **The late hours are the expensive ones.**
  - The world slows as it fills. On eidolon's terrarium, ms per tick goes 6 → 16, i.e.
    8 → 3 sim-min per wall-min.
  - A 6 h terrarium run takes 70–80 wall-min.
  - P1's current job list is 288 sim-hours, about 12–20 wall-hours.

## A faster way to verify an edit

1. **Log what predicts, not just counts.** Every late collapse above showed early in
   these:
   - per species, per minute: living biomass and reserve, dead wood, litter, mean
     root-box water and growth rate;
   - deaths by cause (starved, drowned, aged, eaten);
   - births per lineage.
2. **Check at t = 0, before running anything.**
   - Build a viability table from the founding state: each species' root-box water,
     growth rate, and reserve ÷ upkeep. D1's plant-only autopsy predicted the dry-world
     deaths to the minute this way.
   - Add the R/S/F stay-or-go numbers from the parameter audit.
   - Several past 6 h runs could have been skipped.
3. **Short runs, many seeds, projected forward.**
   - The default check: 8 seeds × 2 h in parallel.
   - Project each stock's last-30-min trend forward, as the ecology search already does
     from 3 h to 6 h.
   - The result is a spread instead of one noisy number, and it arrives before the
     expensive hours.
4. **Stop each seed's run once its verdict is settled.** Stop when:
   - a founder species or lineage hits zero; or
   - growth passes a cap (the search stops at 150 animals); or
   - every indicator stays within ±20 % for 30 min.
5. **Branch slow-clock edits from a saved world.**
   - `voxel_census` already has `save=` and `load=`.
   - Save the baseline at hour 2, once per main commit.
   - An edit to lifespan, defence or decomposition loads that save and runs 60–120 min
     beside the baseline continuing from the same save. That skips founding and starts
     among mixed ages, with breeding under way.
   - Limits: this works only when the edit doesn't bump the snapshot schema, and `load=`
     should also accept parameter overrides.
6. **Keep one long soak, off the critical path.**
   - Run 8 seeds × 8 h on eidolon per merge batch or overnight, with an automatic
     summary.
   - It covers what really is slow: seed banks (2–12 h), the second generation's echo,
     and dead wood.
   - Stop running single-seed 6 h censuses for each edit.

Applied to P1's list, a 2 h cap with early stopping needs at most a quarter of its
288 sim-hours. Wall time falls further, because the early hours run fastest.

## Package (after P1 and WX3 are on main; both edit `voxel_census.rs` today)

**V1** (sonnet-high) makes these changes to the census tooling:
- the default length drops from 6 h to 2 h, and the tool's doc comment cites
  WORKING_POLICY "Verification runs";
- the indicator columns;
- the t = 0 viability table;
- early stopping and projection;
- a multi-seed driver with a summary across seeds;
- `load=` with parameter overrides.
