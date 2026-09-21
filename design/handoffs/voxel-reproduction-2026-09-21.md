---
status: landed in worktree reproduction (3bfe98f..f3348f0); awaiting main
date: 2026-09-21
owner: Fable (orchestration); creature decisions recorded from Wrysk
---

# Reproduction: the frondgrazer gestates, the littershredder lays eggs

Wrysk, 2026-09-21. The voxel fauna's birth rule is a placeholder that was
never replaced: a body at `birth_body` with `birth_cost` of reserve gives
birth that tick, every tick it can, with no interval (`crates/cubarium-voxel-
fauna/src/step.rs`, `births`). On the living generated world that makes 8
browsers into 48 in seven minutes, strips the reachable canopy and kills the
lineage by minute 34 (`voxel-browser-reach-2026-09-21.md`, integration
note). The design already says otherwise: "births after sustained surplus"
and "paid offspring through the existing gestation escrow" (`design/
theoretical-biosphere-2026-09-16.md`, `design/voxel-ecology-sketch-2026-09-
16.md`; the escrow is described in `design/evolution.md` §"gestation").

Decided: **the frondgrazer gives live birth through a gestation escrow,
one offspring at a time, only after a sustained reserve surplus, with an
interval between births. The littershredder lays a clutch of eggs on the
litter it lives in; the eggs are a stationary paid package in the world
that hatches after an incubation interval.** Every new number is a
placeholder recorded in `design/backlog.md` §1; no tuning loop.

Owner: one Opus 5 worker, high effort. Workspace: the git worktree
`/home/wrysk/wryskware/cubarium/.claude/worktrees/reproduction` at e5658af
(it carries the browser reach and its retrained centre). Crates:
`cubarium-voxel-fauna` (body, step, lib, snapshot, tests) and
`crates/cubarium/examples/voxel_founder_autopsy.rs` (egg and gestation
columns); nothing else. Do not touch the main checkout.

## Rules that bind both

- Paid, never free: every unit of offspring organic, mineral and energy
  leaves the parent's reserve; the fauna ledger closes to 1e-9 across
  gestation, birth, laying, incubation and hatching.
- A newborn or hatchling arrives at `body_min` with the rest of its package
  as reserve, at the parent's pose (birth) or the egg's site (hatch), and
  gets a fresh controller from the lineage factory (`FounderFactories`).
- The arena path is unaffected: `set_births_enabled(false)` still disables
  everything here, and the sensing tests must not change.
- Snapshot schema bump (always fresh): the escrow and the eggs are saved.

## Frondgrazer: gestation escrow

Rule, with placeholder constants on `FounderPhysiology` (state your values):
1. **Eligibility.** Body ≥ `birth_body` and reserve ≥ `birth_cost` +
   `surplus_floor`, held continuously for `surplus_hold_s` (a counter in
   `founder_state`; any tick below the floor resets it).
2. **Gestation.** Over `gestation_s` the parent moves `birth_cost` from its
   reserve into an `escrow` on the animal, in equal per-tick instalments,
   with mineral and energy pro rata as a birth does today. The escrow pays
   no upkeep of its own (the parent's maintenance is unchanged); it cannot
   be spent by the parent. If the parent's reserve hits zero mid-gestation
   or it dies, the gestation fails: `gestation_loss_fraction` of the escrow
   is respired (booked), the rest returns to the reserve (or the corpse).
3. **Birth.** At term one offspring is created from the escrow exactly as
   `births` builds a newborn today; then `birth_interval_s` of refractory
   before eligibility can begin again.

## Littershredder: eggs

1. **Eligibility** as above, with the shredder's constants.
2. **Laying.** A clutch of `clutch_size` eggs at once, total organic
   `clutch_size · egg_organic` (with mineral and energy pro rata), paid from
   the reserve in one tick, placed as one `Clutch { site, count,
   organic, mineral, energy, laid_tick, lineage }` record on the animal's
   current support face; laying requires litter on that site (the shredder
   lives in litter) and fails, costing nothing, otherwise. Then
   `birth_interval_s` refractory.
3. **Incubation.** Eggs pay no upkeep. After `incubation_s` each egg
   hatches into a `body_min` juvenile with the egg's remaining mass as
   reserve, at the clutch's site, controller from the factory. A clutch on
   a site that is removed, or covered by water deeper than the founder's
   `drown_depth_m`, is lost to carrion at that site (booked through the
   flora deposit path a death uses). Eggs are counted in the ledger's
   stored totals and in the census as `eggs_by_founder`.
4. Nothing eats eggs yet; the record must make that possible later (a
   site-fixed package with currencies), but no consumer is added here.

## Tests, specified before the implementation

(a) no birth before the hold elapses, even at full stores; (b) the escrow
instalments sum to `birth_cost` exactly and the reserve falls by the same;
(c) a reserve driven to zero mid-gestation loses exactly the loss fraction
and returns the rest; (d) after a birth no second birth inside the
interval; (e) a clutch's currencies equal the parent's reserve drop; (f)
hatching creates `clutch_size` bodies whose summed package equals the
clutch's; (g) a clutch under deep water becomes carrion of the same
currencies; (h) snapshot round trip carries escrow, counters and clutches,
and the old schema is refused; (i) the closed-ledger habitat test still
holds over 200 ticks with both rules active; (j) `set_births_enabled(false)`
suppresses eligibility, gestation and laying.

## Measurement

`voxel_founder_autopsy 60 generated closed` in the worktree with the
built-in defaults, reporting per lineage: alive, born or hatched, eggs
standing, deaths by cause, bites, and the same columns as before, against
the two recorded arms (D3: 33 browser deaths, extinct after 54 min;
reach: 60 deaths, extinct at 34, foliage 17.6 → 8.6). Then
`voxel_census 6 generated closed`: every lineage alive at 6 h, or not, and
the plant counts against the recorded 5-of-6 run. Report the placeholder
values you chose and why, and what the two arms show.

## Rules

Explicit-path commits in the worktree ending with `Co-Authored-By: Claude
Fable 5.1 <noreply@anthropic.com>`; never `git add -A`; do not edit
`design/handoffs/README.md`; tests ≤ 200 ticks by conservation arithmetic;
no bit-identical pins; no changes to feeding, movement, reach, senses or the
seeder; runs may use all cores; `runs/` is disposable; no windows.
`cargo nextest run --workspace --exclude cubarium-gpu` green in the worktree
before returning. Return ≤ 40 lines.

## Integration note (Fable, 2026-09-21, at f3348f0 in worktree `reproduction`)

Commits: 3bfe98f (both rules, fauna snapshot schema 9), be32f79 (the ten
tests as specified), f30c589 (autopsy and census columns, placeholders in
`design/backlog.md` §1), f3348f0 (ledger residuals in the autopsy).
Workspace suite 1,988 green in the worktree. Fable re-ran the 60-minute arm
from the worktree's build and got the worker's numbers exactly.

Placeholders chosen (backlog §1): browser surplus floor 0.005, hold 120 s,
gestation 180 s, loss fraction 0.25, interval 300 s; shredder floor
0.000625, hold 120 s, interval 300 s, incubation 300 s, egg 0.004 organic,
**clutch size 1** — forced, not chosen: the frozen blind body's reserve
ceiling cannot pay two viable eggs; the rule and tests handle any count.

Deviations, accepted: reproduction state on `Animal.reproduction` rather
than the controller-interval `founder_state`; tables on
`SpeciesConfig.reproduction` so the browser founder shares the frondgrazer
species' numbers; egg eligibility priced at the clutch, since the shredder's
`birth_cost` is its whole reserve ceiling; `born` counts hatchlings;
gestation respiration is a fourth respiration split.

**60-minute arm, generated closed world, built-in defaults:**

| | D3 (before reach) | reach only | reach + reproduction |
| --- | --- | --- | --- |
| browser deaths | 33 | 60 | 13 |
| browser peak alive | – | 48 at min 7 | 12 at min 7 |
| browser extinct at | after 54 | 34 | 35 |
| foliage low point | 13.7 | 8.6 | 10.4, recovering to 12.5 |
| shredders alive at 60 min | 15 | 15 | 14 (30 eggs laid, 30 hatched, 0 lost) |

The overshoot is gone and the canopy is never stripped, so the browser's
remaining failure is not overgrazing: 13 bodies starve with 10–12 organic
of foliage standing. The reach and search line stays open. Six-hour census:
frondgrazer 0 from minute 35, littershredder 0 from minute 122 (litter then
climbs to 119.6 uneaten); plants **6 of 6 alive** at 6 h (bloomcrown 33,
umbrellafrond 139, springturf 20, stonecushion 34, velvetpad 2, glowcap 10)
against the recorded 5 of 6.

Main: the chain e5658af..f3348f0 waits in this worktree because the main
checkout carries another agent's uncommitted edits to
`crates/cubarium/src/voxel/mod.rs` and, as of this note, an uncommitted copy
of e5658af's asset swap. Cherry-pick once main is clean.
