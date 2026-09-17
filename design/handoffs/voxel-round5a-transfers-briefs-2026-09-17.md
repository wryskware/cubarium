---
status: leaning
date: 2026-09-17
owner: Fable
---

# Voxel round 5a: bounded food transfers, the substrate every consumer needs

Astra's round 7 (`design/7_Research/astra-voxel-first-wave-review-2026-09-16.md`,
"## Round 7") clears the five-producer substrate for a first consumer prototype
and says how it should start: "with explicit bounded food transfers and the
organic/mineral/energy ledger, leaving population targets and carrying capacity
unclaimed." The biosphere (`design/theoretical-biosphere-2026-09-16.md` §5,
"Candidate minimum communities") offers two first consumers, a frondgrazer on a
grazed meadow or a glowcap in a decomposer grove, and says that before voxel
animals exist "a bounded experimental harvest of reachable foliage can test only
the producer response". This round builds exactly that layer, species-agnostic,
so that either consumer can be the next round and the choice does not block.
**No animal body, no movement, no fungus metabolism, no population.**

Direction (Wrysk, 2026-09-17): simulation and ecology solid for an ambient piece;
no game hooks.

## What the flora layer gains

All in `crates/cubarium-voxel-flora`. Names are suggestions; keep them if
nothing better presents itself.

1. **Withdrawals** (a consumer eats): three public methods on `Flora`, applied
   between ticks like commands, each returning what was actually taken so a
   consumer layer can book it in:
   - `take_foliage(site, want) -> Option<Taken>` from the stand on that site:
     at most its foliage, never wood or reserve; mineral by the same fraction
     rule every other outflow uses (`pull_mineral`); energy at the tissue's
     density.
   - `take_dead_wood(site, want)` and `take_litter(site, want)` from the
     ground's dead pools, mineral and retained energy pro rata.
   - `Taken { organic, mineral, energy }`. Each withdrawal is booked to new
     ledger terms `consumed_organic_out`, `consumed_mineral_out`,
     `consumed_energy_out` (named boundary flows, like `removed_*`), so the
     three residuals hold to the bit with consumers present.
2. **Deposits** (a consumer dies or excretes): `deposit(site, Deposit { kind,
   organic, mineral, energy })` with `kind ∈ { Carrion, Litter }`. Carrion is a
   **new ground pool** (`Ground::carrion`, `carrion_mineral`, `carrion_energy`)
   with its own decomposition rate `FloraConfig::carrion_decomposition`
   (placeholder, faster than litter; backlog row), decomposed in the existing
   `decompose` phase from the tick-start snapshot exactly as litter is, organic
   respired out and mineral released to the site pool at the same fraction.
   Litter deposits join `Ground::litter` through the existing cap rule. Booked
   as `deposited_organic_in`, `deposited_mineral_in`, `deposited_energy_in`.
   A deposit on a site with no `Ground` provisions one (the lazy
   `initial_mineral` rule applies and is booked as today).
3. **Reach** (food above reach does not feed a ground browser; biosphere §6):
   `reachable_foliage(view, from: Site, reach: Reach) -> Vec<(Site, f64)>`,
   sorted by site, listing stands whose crown (the model's own `crown_of`)
   has at least one cell within `reach.horizontal` voxels in wrapped `x` and in
   `z` of the eater's support face and whose **lowest** crown cell is at most
   `reach.up` voxels above that face, with the foliage each holds. Pure
   geometry, no line of sight yet.
4. **Producer response** needs no new rule: a grazed stand's foliage drops,
   income drops with it, and regrowth follows the existing `foliage_rate` and
   reflush-from-reserve rules. Say so in the module doc rather than adding a
   "recovery" mechanic.

Phase order: withdrawals and deposits happen between ticks, so a tick's
decomposition sees deposits from the previous inter-tick, consistent with the
tick-start snapshot rule already in `step`. The ledger's expected totals gain
the four new named flows.

## The harvest study (producer response only)

In `examples/two_producers.rs`, a `harvest <s> <rate> [seed] [noise]` mode: two
arms from one conditioned world (plant-only; plant plus a scripted harvester
that, every tick, takes up to `rate · dt` of foliage from stands reachable from
**three predeclared support sites** with `Reach { horizontal: 2, up: 1 }`, and
deposits nothing), same terrain and rain. Harvest stops at the halfway point.
Report per species the foliage, reserve and wood trajectories every 100 s, the
fraction of harvested stands that recover full foliage before the end, deaths
in each arm, `consumed_*` totals, and the residuals. Two reports: springturf
patch and bloomcrown patch. This is the biosphere's producer-response probe and
nothing more: no animal viability claim, no carrying capacity. 400 coupled
seconds in `--release`; say the wall time.

## Tests to leave behind (short function tests only, new `tests/round5a.rs`)

- Taking more than a stand holds takes exactly its foliage and leaves wood and
  reserve untouched; the stand's mineral falls by the fraction rule; the ledger's
  `consumed_*` equal what was returned; residuals at noise.
- Taking from an empty site or a site with no stand returns `None` and books
  nothing.
- Dead-wood and litter withdrawals carry their pro-rata mineral and energy.
- A carrion deposit conserves mineral exactly through decomposition to the
  site pool and respires its organic matter; a litter deposit obeys the cap.
- Reach: a crown two voxels above a `up: 1` eater is not listed; one across the
  `x` seam is; results are sorted by site.
- A stand stripped of foliage each tick for 200 ticks has a falling reserve
  and no wood growth; the same stand left alone after 50 ticks of stripping
  regrows foliage (assert direction, not a survival outcome).
- Nothing in `tests/round3.rs` and `tests/round4.rs` changes.

## Not in this round

The consumer itself (body, reserve, movement, births, drowning); fungus
metabolism; line of sight; dung as a separate pool; presenter glyphs for
carrion (a backlog row). The replacement-control study (Astra R7.2) is its own
brief.

## Package

**M — transfers**, Opus high, main checkout, after package L (Astra R7
repairs) has landed, because both touch `two_producers.rs`. Files:
`crates/cubarium-voxel-flora/**`, `design/backlog.md`, the experiment note's
new section "Round 5a — 2026-09-17". Commits: withdrawals + ledger; deposits +
carrion; reach; harness `harvest`; the study note. Explicit paths only.

## Rules

Fast iteration; explicit-path commits; no cargo fmt; no knob tuning beyond
named placeholders; no long tests, no pinned hashes; always fresh; do not touch
`design/handoffs/README.md`, the cube, `crates/cubarium-gpu/**`,
`crates/cubarium/src/sink/**`, or the core crate unless a query is missing
(then add it with a test and say so); never HashMap iteration.
