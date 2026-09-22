---
status: decided (by Fable under Wrysk's delegation, 2026-09-21: "just make sensible decisions for me on everything"); not yet a canon ledger entry
date: 2026-09-21
owner: Fable; Wrysk may overturn any line
---

# Organism decisions, 2026-09-21

The nine choices the systems audit put to Wrysk
(`design/7_Research/organism-systems-audit-2026-09-21.md`, "Decisions for
Wrysk"), decided so the packages in its §10 can be written. Each line says the
choice, why, and what it visibly changes. Placeholder numbers stay placeholders
and go to `design/backlog.md`; none is tuned here.

## 1. Adult size and growth

**Browser adult 0.375 m long, shredder adult 0.19 m long; the model body is the
drawn body.** The ladder's manifest lengths, not the anatomy document's doubled
shell. A 0.75 m animal in a 20 m ring is a horse in a garden; the ladder was
sized against the panel and the art kit. **Body dimensions grow with structure:
length ∝ (body / body_max)^(1/3)**, width and height in the adult's proportions;
so a newborn browser is 0.46 of adult length. Eye and mouth anchors are fractions
of body height, so they grow with it. The manifest's cruise reference and stocks
do not change with this; growth changes geometry only.

## 2. Feeding anatomy

**One physical mouth band in metres above the surface the animal stands on:
`[0, 1.33 × body_height]`**, horizontal reach 0.25 × body length ahead of the
footprint, as today. For the adult browser (height 0.1875 m) the ceiling is
0.25 m; the raised neck of the dossier is that 1.33. No rearing, no climbing,
no posture change. A low browser: floor tissues are its food and the canopy
escapes it, which is the succession story the biosphere wants.

## 3. Diet breadth

**Browser: every vascular species' foliage inside the band**, keeping today's
broad acceptance; velvetpad and stonecushion stay on the menu (the proposal's
exclusions would have removed 58 % of what the D3 browser ate). Glowcap caps are
not browser food: they are fungal tissue. **Shredder: litter, glowcap cap tissue,
and carrion**, a detritivore with three detritus foods; the cue becomes a
detritus field (litter + carrion + cap tissue at the face) with the litter
field's transport. Yield per food class is an authored placeholder (backlog).
Carrion finally has a consumer.

## 4. Layer state

**Explicit remaining tissue per layer**, summing to the stand's foliage. A bite
withdraws from the reachable layers; regrowth fills layers bottom-up; stage
transitions and senescence move existing tissue between layers and never create
capacity. Withdrawal books organic, mineral and energy exactly as today.

## 5. Meadow and succession

**Adult bloomcrown keeps a renewable basal rosette (25 % of its foliage) for its
whole life**: the floor food that makes a grazed meadow. **Seedlings of every
woody species have their own physical height, a ground rosette no taller than
0.125 m**, overriding the interpolated crown height until the juvenile stage.
**Umbrellafrond and vaulttree escape at the seedling → juvenile transition;
lanternberry escapes when its lowest foliage clears the band.** Turf, pads and
cushions are floor food at every size. Size-selective seedling mortality is
accepted as real ecology; canopy refuges for seedlings come from the existing
light gates, not from a rule.

## 6. Sensory anatomy

**A fixed fan with physical anchors: the eye at 0.8 × body height above the
standing surface, in metres; pitches −40, −20, 0, +20, +40 degrees**, the same
three yaw sectors and three yaw offsets, range 2 m. Sector aggregation keeps the
observation vector at 37 inputs, so the policy contract keeps its shape while
the fan can see basal food at the feet and a crown above. **The encounter
description is shared: a ground pool is not a wall** unless its physical volume
stands higher than the eye; what a body can see, touch, stand on and eat at a
location comes from one physical query. No foliage odour for the browser now;
taste at the mouth stays. The shredder keeps its chemical field, now detritus.

## 7. Physical scaling ambition

**Retain the abstract organic-unit model with authored geometry.** No kilograms,
no allometric exponent; the only length–mass relation is the cube-root
convention in §1, and it changes geometry, not rates. Rates remain backlog
placeholders.

## 8. Starting-world guarantee

**Establishment plus an initially connected, renewable consumer diet, as an
acceptance check**, in the spirit of the hydrology rule (reject the world, do not
restock it): a seeded world is accepted only if each animal lineage's founders
stand in a walkable component holding edible stock of at least N founder-hours
of upkeep with living producers of it, N a backlog placeholder. Rejection
re-draws the seed like the lake gate does.

## 9. Scope of the next package

**The observer first (audit §10 package 0)**: the physical encounter contract
written down and an edible-stock observer that reports, on today's model and
under the band in §2, what share of standing foliage is reachable, seen and
route-connected at seeding and at six hours, per preset. Then §10's order:
physical geometry and units (including the shade-area units bug), layer stocks,
diets, startup acceptance, training refresh, coupled assessment. Bellwing after.

## What this does not decide

Numbers in the backlog; the canon ledger (Wrysk promotes these lines when he
wishes); the art. The anatomy document's species profiles are edited to these
decisions in the layers package, not here.
