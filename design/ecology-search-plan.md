---
design_status: exploration
last_reviewed: 2026-09-14
---

# Whole-ecosystem parameter search

Wrysk's direction: use fast headless simulations and genetic search to discover
parameter combinations that sustain an interesting ecosystem. The previous
stationary-prey apex screen only exercised mechanisms; it cannot assess whole
ecology. Search results guide live iteration and do not gate access to features.

## Bounded first experiment

Run the actual core simulation without rendering. First measure ticks/second
and peak memory for one seeded world. Declare the candidate count, seed count,
tick horizon, worker concurrency, wall-time limit, and output limit before a
search starts. Use one normal build cache and compact result rows; no per-tick
logs, capture archives, or executable copies.

Search a small explicit set of existing ecological parameters jointly: producer
renewal, nutrient recycling, prey feeding and maintenance, reproduction costs,
and predator feeding/maintenance and emergence conditions where configurable.
Initial bounds are hypotheses, not established viable ranges. Avoid inventing
new biology just to create adjustable knobs. All candidates get comparable
initial external resources and the same training seeds; do not restock failing
worlds during evaluation. Include inherited traits and ordinary prey activity.

Use multiple outcomes rather than total organism count alone: continued plant
and prey turnover, reproductive lineages, ecological variety, recovery after
predator pressure, and time without terminal collapse. Distinguish active from
dormant predators and births from descendants that mature and reproduce.
Dormant-only survival, immortal unfed bodies, or one runaway species should not
count as sustainable variety. Apex activity can be episodic; permanent active
predator occupancy is not the target.

Start with a small population of parameter candidates, bounded mutations and
crossover, elitism, and a hard generation/evaluation cap. Report the component
metrics and nondominated candidates, so a convenient scalar ranking does not
hide why a candidate won. Retest promising candidates on held-out seeds and a
longer horizon only in a separately bounded follow-up. A short successful run
is a screening result, not proof of indefinite sustainability.

## First implementation handoff

Build one minimal headless search harness around the real core. Discover and
document existing configurable parameter fields and defensible initial bounds.
Use a fixed seed schedule and deterministic search RNG; persist parameter
values, component metrics, seed, horizon, build ID, elapsed time, and failure
reason. Validate replay and evaluation-budget enforcement. Measure one baseline
run before choosing a small smoke-test budget. Return the throughput and exact
command for a subsequent capped search. No broad search, visual work, live
parameter replacement, or historical transcript review in this milestone.
