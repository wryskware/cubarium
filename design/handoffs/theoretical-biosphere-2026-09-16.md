---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Handoff: a theoretical biosphere for Cubarium, designed top-down

Wrysk's request (2026-09-16): design a food web and biosphere for Cubarium from
theory, **not limited to the plants and animals the code has today**, so the world
can be shaped toward it. Keep in mind that Cubarium is a game about making
ecological systems: a player will eventually unlock and build a progressive
ecological web and steer its evolution toward healthy or unhealthy states. That
second part is later work; the primary angle stays **an ambient living world on a
desk**. Consider the game angle even if the honest recommendation is to ignore it
until more is built.

This is a theory deliverable. No code, no tuning, no art. It records no accepted
decision; it is the document we will then try to shape the world toward.

## Read first

1. [Terrain and ecosystem proposal](../terrain-and-ecosystem-proposal-2026-09-16.md)
   §4 (water and physical model) and §5 (habitat communities). §5's roster is a
   sketch you are free to discard; §4 is the substrate you must design on.
2. [Reconsidering the food web](../ecological-niches-reconsideration-2026-09-15.md):
   resources by identity, bodies establish diets, interdependence through
   consequences, space and population. These are constraints on *how* a web may
   work, not on *which* web.
3. [Voxel ecology sketch](../voxel-ecology-sketch-2026-09-16.md): the current
   six placeholder roles, what terrain exposes, and the first coupled experiment
   (two producers) now running. Read it to know what exists; do not treat its
   roster as given.
4. [Game roadmap](../game-roadmap-2026-09-16.md) §1–4: the player loop, the
   "barren terrace becomes a habitat" story, progression and the science tree.
   Only for the secondary question.
5. [Ecology v1 contract](../ecology-v1-contract.md) §3–§5, skimmed: the
   accounting discipline every organism has to satisfy (paid maintenance, growth,
   reproduction and propagules; energy dissipated at every transfer; nutrients
   cycled; no stock created by decree). Adopt the discipline, not the equations.
6. [Research foundations](../7_Research/terrain-ecology-foundations-2026-09-16.md):
   primary sources already collected. Cite them or better ones; evidence, not
   authority.

Do not ingest the rest of the archive.

## The substrate you are designing on

This is fixed for the purpose of the exercise. You may ask for capabilities the
substrate lacks; list them separately as requests, with the ecological reason.

- **Space.** A horizontally periodic voxel strip, presently 128 wide, 24 deep and
  48 high at 0.25 m per voxel; the full version is deeper. One landform per world:
  low ground and a receiving basin at the front, hills peaking at the far edge,
  a ridge, terraces, soil pockets in rock, hard strata that perch water. A later
  cube version wraps one landscape around a bedrock core. Nothing here is Earth
  scale: think of a terrarium the size of a room, not a landscape.
- **Materials.** Air, bedrock, rock, soil, each with pore capacity and
  permeability. Terrain is editable by command (by the player later).
- **Water.** Prescribed rain onto sky-exposed surfaces; infiltration before
  runoff; free water that falls and pools; soil pore water; drainage to a
  connected aquifer; a **water table** that saturates soil below its head and
  seeps ponds into hollows below it; a named spring and an outlet; evaporation
  as a named loss. Result: low ground is wet, ridges drain to field capacity,
  and sustained rain floods. A later mode recycles evaporation through a finite
  atmosphere back into rain. Water carries no nutrient and makes no food.
- **Light.** Geometric sky visibility from terrain (a cosine-weighted hemisphere
  fan) times canopy attenuation owned by the plants. There is no day/night in the
  ecology yet; the roadmap keeps presentation lighting constant while an
  ecological day may run underneath. No temperature model exists.
- **Sessile life.** One stand model: foliage, wood and reserve per stand on a
  support face; paid maintenance and growth; dieback under unpaid maintenance;
  death to litter and located dead wood; paid propagules to nearby support faces
  gated by an establishment predicate; roots draw pore water through one bounded
  withdrawal; light income from geometry and shade. Species are trait presets on
  this model (shade response, drought and flood tolerance, rooting depth, growth
  versus persistence, reproductive investment, dispersal hop, crown geometry).
  Adding a sessile species costs a preset, not code; adding a *mechanism* (fruit,
  a non-photosynthetic metabolism, a long-distance dispersal vector) costs code.
- **Animals** (to be rebuilt on the voxel world; the cube's versions are the
  prior art): body-defined diets with a shared mouth and gut budget, per-resource
  yield and hard exclusions; a motor budget in body lengths per second, to gain
  slope, step height and wading; reach and sight from voxel geometry; paid
  reproduction with gestation escrow and paid juvenile growth; recurrent-network
  controllers that choose where to go, what accessible food to try, whether to
  rest, court or reproduce. The body enforces what is possible; the brain cannot
  override digestion.
- **Scale.** Hundreds to a few thousand sessile stands; animals in the tens to
  low hundreds. Supported biomass comes from sustained accessible production, not
  from a desired head count.
- **Readability.** Pixel art at 4 px per voxel, seen across a room and up close.
  Every role must be recognizable by silhouette and motion at that scale, and
  every coupling you rely on should be *visible*: a grazed gap, a shaded-out
  patch, a glowing log, a shoreline that moves with the water table.

## What is not a constraint

- The current roster (bloomcrown, umbrellafrond, glowcap, frondgrazer,
  littershredder, lanternjaw). Names are placeholders for roles; keep, rename or
  drop them.
- Earth taxonomy. This is an alien terrarium. Use Earth ecology as the source of
  reasoning about what persists and why, not as a species list. Invented biology
  is welcome when it is paid for: a glow, a chemosynthetic mat at the spring, a
  plant that traps and digests, an animal that farms fungus. State the energy
  source and the cost of every invented trait.
- Balance. Local losses, succession and boom-and-bust are acceptable and
  interesting; permanent sameness is not the goal. Do not design for stasis.

## Deliverable

One document, `design/theoretical-biosphere-<date>.md`, status `exploration`,
a few thousand words, tables and one or two diagrams, with these sections.

1. **Energy and gradients.** Where energy enters (light; anything else, and at
   what cost) and how it dissipates. The abiotic gradients this terrain actually
   produces (light, pore water, standing water, slope, substrate, cover, depth in
   the habitat) and which of them the biosphere should be organized around.
2. **The web.** Functional guilds and a food web with several channels rather
   than one tall chain: producers, reproductive structures, litter and dead wood,
   living prey, carrion, waste, fungi and microbes, mineral nutrient. Every arrow
   is a transfer of a resource with identity; say which organisms can and cannot
   use each one and why. Include the non-feeding network: shade, cover, soil
   modification, pollination or seed carriage, habitat construction.
3. **Cycles and disturbance.** Nutrient and water cycles as they close on this
   substrate; the disturbance regimes it can generate (drought as the table drops,
   flood, a tree's death, a grazed opening, a burrow collapse) and the succession
   story that follows each. Say what a keystone is here and what redundancy keeps
   one loss from switching the world off.
4. **The palette.** Roughly 8–12 sessile and 6–8 animal roles, as a table: role,
   what it eats by identity, what it needs from terrain, life history and
   reproduction, dispersal, death, what its removal would visibly do, and what a
   viewer sees (silhouette, motion, colour accent). Mark each one as expressible
   on the stand model or the animal body as they exist, or as needing a named new
   mechanism.
5. **Why it persists.** The reasoning that this web can hold on a strip this
   size with a water table and prescribed rain: which rates matter, where the
   fragile points are, what a sustained accessible production budget looks like,
   and which **minimal subsets are viable on their own**. That last list is our
   build order; make it explicit, starting from the two producers we have.
6. **Requests to the substrate.** Capabilities the biosphere needs that the
   physics and models lack, each with the ecological reason and a note on
   whether a cheaper stand-in exists.
7. **The game angle, secondary.** How a player could unlock and build this web
   progressively; what "healthy" and "unhealthy" would mean measurably (not
   population caps); what interventions would steer evolution and what they
   would cost; and which of this to ignore until more is built. "Revisit when X
   exists" is an acceptable answer for any part of it, provided X is named.

## Rules for the theory

- **No energy by decree.** Every organism pays maintenance, growth and
  reproduction from what it actually acquires; reprocessing waste or carrion
  never recharges its original food value; microbes make resources accessible by
  consuming some, never as a bonus for waiting.
- **Resources keep identity.** Foliage, fruit or seed, litter, dead wood, living
  prey, carrion and waste are different things with different consumers. Do not
  merge them into "biomass".
- **Trade-offs, not penalties.** Broad diets and wide tolerances cost something
  measurable (throughput, speed, reproductive investment). Choose the smallest
  mechanism that yields a trade-off; do not stack penalties until specialists win
  by construction.
- **Geography does the work.** Habitat differences come from light, water, slope
  and substrate, never from a hidden field or a preferred destination. If a
  species' niche cannot be stated as a predicate on terrain and neighbours, it
  has no niche.
- **Orders of magnitude only.** Rates and sizes as ratios and orders of
  magnitude with a reason; no tuned constants.
- Cite sources for non-obvious ecological claims. Mark speculation as such.

## Exclusions

No implementation, no code reading beyond the linked documents, no changes to
the water or terrain physics (request them instead), no art direction beyond
what a role must look like to be read, no campaign or economy design.

## Return

The document above, plus a short note back listing: the three things in the
existing direction you would change first, the one experiment that would most
quickly show whether the proposed web is viable here, and anything in the
substrate you consider a mistake.

Suggested model for this handoff: a frontier reasoning model on a fresh thread
(Astra through the codex workflow with `--fresh`, or a Fable session). It is a
synthesis, not a lookup.
