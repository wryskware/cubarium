---
status: landed
date: 2026-09-20
owner: Fable (orchestration); Wrysk decides what changes afterwards
---

# Why the seeded habitat collapses: two read-only diagnoses

Evidence: `design/7_Research/voxel-census-2026-09-20.md` (two six-hour
censuses of `crates/cubarium/examples/voxel_census.rs`). Every seeded plant
except stonecushion dies on a fixed schedule that animals do not change, no
stand ever establishes, and the founders boom to 65 by minute 5 and are all
dead by minute 30 while litter rises. Both packages are **diagnoses**: they
must end in a stated cause with evidence, not in a changed constant. Any
change beyond instrumentation is Wrysk's decision and is proposed, not made.

Shared rules: explicit-path commits ending with `Co-Authored-By: Claude Fable
5.1 <noreply@anthropic.com>`; never `git add -A`; do not edit
`design/handoffs/README.md`; tests ≤ 200 ticks; no bit-identical pins; runs
may use all cores; disposable outputs under `runs/`. Return ≤ 40 lines with
the cause, the evidence, and the one change you would propose.

## D1 — why the plants die here and not in the study arena

Owner: Opus 5, high. Crates: `cubarium-voxel-flora` (read), `cubarium-voxel`
(read), `crates/cubarium/src/voxel/{habitat,scene}.rs` (read; a new example
under `crates/cubarium/examples/` is the only write).

Start from the census timeline: springturf 16 stands → 0 at minute 16;
glowcap at ~2.5 h; velvetpad 3 h; bloomcrown and umbrellafrond both at
minute 250; stonecushion never. Flora births stay at 62 for six hours.

1. For each seeded species, read the establishment/survival predicate and
   the death rule in the flora crate (`design/handoffs/voxel-producers-briefs-2026-09-16.md`,
   `voxel-round3-briefs-2026-09-16.md` and the presets brief name them) and
   the study arena that kept the same species alive for 1.8–5 h
   (`design/7_Research/voxel-round3-experiment-2026-09-16.md`, the
   replacement harness under `crates/cubarium-voxel-flora/examples/`).
2. Write `examples/voxel_plant_autopsy.rs`: seed the authored habitat exactly
   as `habitat::seed` does, then per stand print at seeding and every
   simulated minute until it dies: species, site, water depth at the site,
   the water-table or wetness quantity its predicate reads, light or sky
   visibility, wood/foliage/reserve, and which clause of its own survival
   rule is failing. Nothing here may change a rule.
3. Answer, per species: what kills it (drowning, drought, light, the
   half-wood start below a maintenance floor, or a seeding site its own
   predicate would refuse), and why the same species lived in the study
   arena (different water, soil, light, or start size). Answer why no
   germination ever happens (no seed bank, bank never passes the predicate,
   or no propagules paid).
4. Say which one change you would propose (seeder placement, a different
   start size, the authored terrain's water) and what evidence would show it
   worked, without making it.

## D2 — what kills the founders in the first half hour

Owner: Opus 5, high. Crates: `cubarium-voxel-fauna` (ledger and one census
hook), `crates/cubarium/examples/` (a new example). Do not touch
`cubarium-search`, the seeder, or any birth/feeding constant.

1. **Death cause in the ledger.** Add per-cause death counters to
   `FaunaLedger` (starved, drowned, any other path that removes a body),
   per founder lineage where the body has one, with a short test for each
   cause. This is instrumentation; the totals must still equal `deaths`.
2. Write `examples/voxel_founder_autopsy.rs`: the seeded founder world with
   the live senses field (see `examples/voxel_census.rs` for the setup),
   stepped 36,000 ticks (30 min). Every simulated minute print: per lineage
   alive, born, deaths by cause, mean body and reserve, bites, and for every
   living animal the distance to its nearest edible food (litter site for
   the shredder, foliage stand for the browser) against its sensed reach
   (1.5 m blind, 2.0 m browser; `cubarium_search::es::voxel::task::sensed_radius_m`
   holds the provenance but do not depend on that crate — restate the two
   numbers). At each birth print the newborn's body, reserve, and distance
   to food.
3. Answer: what the 65 deaths are by cause and lineage; whether newborns
   are born out of reach of any food; whether the boom starves the parents
   (the blind founder pays a full reserve per birth) or crowds the eight
   litter tiles; and how long a newborn at `body_min` with
   `birth_cost − body_min` reserve can live without eating, against how far
   it can walk in that time at its pace.
4. Propose one change (a birth cooldown, a newborn placement rule, a reserve
   floor after birth, or a seeding density) with the evidence that would
   show it worked, without making it.

## Result (Fable, 2026-09-20)

Both diagnoses landed; findings and proposed changes are recorded in
`design/7_Research/voxel-census-2026-09-20.md` ("Diagnoses"). Decisions
(water budget of the ambient world; the blind heuristic; the same-height
motion rule) are Wrysk's.
