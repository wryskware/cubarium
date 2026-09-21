---
status: landed
date: 2026-09-20
owner: Fable (orchestration); decisions recorded from Wrysk
---

# A closed water cycle for generated worlds (route B)

Wrysk, 2026-09-20: before anything else in generated worlds, a viable water
cycle. Decided: **route B** — a closed cycle with a lumped atmosphere store —
now; a spatial atmosphere with clouds (route C) is a future layer on top of
B's store. A drought lock (water settling where evaporation cannot lift it) is
a **feature, not a bug**: in the game a player brings reserves back into the
active ecology with tech; for the ambient piece we **reject generated worlds
whose hydrology is not viable, or add water by hand** until it is (a user
lever in the basic control UI, like the flat world's "make it rain"). No knob
tuning of plants; the plants' pore bands are the target the water must meet.

Evidence that motivates this: `design/7_Research/voxel-census-2026-09-20.md`
(D1: the authored world has no pore water, every stand's income is zero; the
study arena lived on rain 2e-4 m/s, an open outlet and a charged aquifer,
i.e. an open flow-through budget).

Owner: one Opus 5 worker, high effort (the water schedule, the ledger, and a
day-long measurement). Crates: `cubarium-voxel` (water.rs, ledger.rs,
config.rs, world.rs, examples/basin.rs), `cubarium-voxel-sim` (only if a
schedule phase must be added), `crates/cubarium/src/voxel/mod.rs` (the
stdin control lever). Read `design/terrain-and-ecosystem-proposal-2026-09-16.md`
§"Water" (rows on evaporation, atmospheric recycling, and "reserve water
before distributing a rain event"), `crates/cubarium-voxel/src/water.rs`
(`rain`, `evaporate`, `outlet`, `spring`, `water_table`), `ledger.rs`, and
`crates/cubarium-voxel-flora/examples/two_producers.rs` (the study's water
budget: `HARNESS_RAIN_M_PER_S`, outlet 0.05, head basin floor + 1 m).

## What to build

1. **An atmosphere store** (`atmosphere_m3` on the world, saved with it).
   Evaporation and transpiration deposit into it instead of leaving the
   world; the ledger keeps `evaporation_out` and `transpiration_out` as the
   flows *into* the store and adds the store to `expected_stored`, so the
   residual stays zero and the old open-budget worlds refuse to load (always
   fresh: bump the world snapshot schema). A `Config` switch selects closed
   or the existing open budget; the open budget is unchanged for existing
   tests and the study harness.
2. **Showers from the store.** Rain draws from the atmosphere: when the store
   exceeds a threshold (a fraction of the world's total water, config), a
   shower runs at `rain_m_per_s` until a shower volume (config) has fallen or
   the store empties; between showers no rain. Reserve the volume before
   distributing it (proposal). Spatially uniform in this package; route C
   will later choose where. Report shower starts in the ledger as a count.
3. **The return loop.** The outlet's export goes into the same atmosphere
   store, or into a second lumped `reserve_m3` that the spring draws from —
   the worker chooses, states why, and books it. Nothing leaves the world in
   the closed budget except through `displaced_out`.
4. **The user lever.** The ambient run's stdin control gets a command that
   adds water to the atmosphere store (the "make it rain" analogue), booked
   `user_in`; `r` and `a` keep their meanings.
5. **Viability check.** A function that, for a world after a warm-up, reports
   whether the cycle is viable: stored water bounded over the window, at
   least N showers, and the fraction of soil columns whose pore water is
   inside the seeded species' establishment bands. The ambient run prints it
   at start; it does not yet reject (that is a later decision), it reports.

## Measurement

- `examples/basin.rs` (or a new `examples/water_cycle.rs`) on the **generated**
  world (`World::new`, default noise seed, then two more seeds) for 24
  simulated hours, closed budget, with the study's rain rate as the shower
  rate: print every simulated 10 minutes stored, free, pore, aquifer,
  atmosphere, showers so far, ledger residual. Show the cycle settles into a
  bounded oscillation, or that it locks dry, and at what threshold.
- Then the six-hour `examples/voxel_census` on the generated world with the
  closed budget: does every seeded species still stand at six hours, and do
  flora births exceed the 62 seeded? (`habitat::seed` currently seeds the
  authored scene; add a way to seed the generated world, or use
  `--scene generated` if the seeder already accepts a world.) Note that the
  seeder's site proxy never consults the establishment predicate (D1); if
  that alone kills the run, say so and stop rather than fix the seeder.

## Rules

Explicit-path commits ending with `Co-Authored-By: Claude Fable 5.1
<noreply@anthropic.com>`; never `git add -A`; do not edit
`design/handoffs/README.md`. Always fresh. Tests ≤ 200 ticks and by
conservation arithmetic, no bit-identical pins. No plant or animal constant
changes. Runs may use all cores; outputs under `runs/` are disposable.
Return ≤ 40 lines: commits, the 24 h table for the three seeds (bounded or
locked, thresholds), the six-hour census line, the viability report, and
the evidence behind each conclusion. Fable integrates.

## Integration note (Fable, 2026-09-20, at 72101aa)

Landed: 197d4b6 (atmosphere store, showers, return loop, ledger, config
switch, world schema 4), df73de5 (`AddAtmosphere`, stdin `h M3`), 5417e2f
(`viability::measure`, printed at start), 63ebecd (`examples/water_cycle.rs`;
`voxel_census [HOURS] [authored|generated] [open|closed]`), 4d3e048, a7849fc,
72101aa. Workspace suite 1,959 passed, 1 skipped; the flora study harness is
unchanged. Worker's decision, accepted: the outlet returns to the atmosphere
(one store, one threshold, one residual, one thing route C replaces) rather
than to a second reserve.

**24 h, three generated seeds, closed budget** (shower rate 2e-4 m/s,
evaporation 1e-4, trigger 0.02, shower 5 m³, aquifer at floor + 1 m; total
water 218.40 m³). Read from the CSVs by Fable: stored water stays within
192–218 m³ all day; first-half to second-half mean drift +0.25 %, +0.36 %,
+0.67 % of total; 456, 663, 653 showers; residual 1.4–1.5e-5 m³ at the end,
i.e. 6e-8 relative, growing linearly with throughput (f64 accumulation over
1.73 M ticks; the ≤ 200-tick tests hold 1e-9). All three BOUNDED.

**Lock threshold** (seed 1, 3 h sweep of `shower_trigger_fraction`): 0.10 →
24 showers, 0.20 → 6, 0.24 → 2, still bounded; 0.26 and above → 0 showers,
LOCKED DRY. Without rain the atmosphere plateaus at 25.3 % of total water,
so any trigger above that is never reached. Intermittent weather lives in
0.10–0.24; the default 0.02 is a placeholder that rains nearly back to back.

**Six-hour census, generated world, closed budget** (worker's run, reproduced
exactly by Fable at 72101aa): at 6 h bloomcrown 42, umbrellafrond 149, springturf 34,
stonecushion 34, velvetpad 27, glowcap 0 (first zero at minute 307); flora
births 376 against 62 seeded, 90 deaths. Same build on the authored open
world: only stonecushion's 6 alive, births frozen at 62 (D1 reproduced). The
budget, not the build, changed the outcome. Fauna still extinct by minute
30 (D2's blind heuristic, untouched). Glowcap is the decomposer; its gate is
dead-wood substrate, not pore water, and it was not chased here.

**Viability line as the ambient run prints it** (reproduced by Fable):
VIABLE, stored 205–211 m³ (drift −1.31 % bounded), atmosphere 7–13 m³,
residual 6e-10; in band: bloomcrown, stonecushion, glowcap 100 % of 3,044
soil columns; umbrellafrond 9.4 %, springturf 13.9 %, velvetpad 12.5 %.

Left open, by the worker and agreed:
- Evaporation is an experiment value, not canon. It must sit below the
  shower rate because `evaporate` runs right after `rain` in the tick and
  lifts fresh rain before it infiltrates; at 4e-4 the soil never recharged.
  The engine of the cycle is the outlet return, not evaporation. Ordering
  evaporation after infiltration is a model question for later.
- Seed 1 loses ~0.1 m³ of store per 6 h to the aquifer; a longer watch would
  say whether that ends in a lock. Trigger sweep on seed 1 only.
- The seeder still never consults the establishment predicate.
