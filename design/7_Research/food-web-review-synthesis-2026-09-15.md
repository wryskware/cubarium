---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Food-web review synthesis

Read [Codex's independent comparison](codex-independent-food-web-comparison-2026-09-15.md)
for the assessment frozen before opening
[Fable's comparison](fable-food-web-comparison-2026-09-15.md).
This synthesis records the subsequent assessment. It does not modify either
independent document or promote any choice to canon.

## What changed after reading Fable's comparison

Fable reports later telemetry with 81 animals, abundant mineral nutrient and much
less material in bodies than its initial diagnosis assumed. It retracts the strong
nutrient-lock-up explanation and places more weight on initial litter. Those are
useful corrections consistent with the observed boom followed by starvation.

However, changes in a resource stock do not identify all its contributing flows.
An approximately constant late detritus stock can coexist with substantial turnover.
It does not establish that ongoing transport is secondary. Likewise, mean vegetation
of 0.18 per cell versus a local feeding threshold of 0.2 does not establish that
individual cells are pinned at that threshold. Food maps and flow measurements,
not averages alone, should establish those mechanisms. Fable's causal claims remain
hypotheses for discriminating tests.

## Shared direction, with qualifications

- Assess reproduction, juvenile growth and food renewal together. Measure the
  consumer/recovery time ratio without treating one as a universal stability boundary.
- Account for initial food separately from renewable supply, with visible resources.
- Keep specialization costly and allow limited useful generalism. Existing mouth
  allocation already trades intake rates; its ecological strength needs testing.
- Keep the RNN responsible for decisions and physiology responsible for feasibility.
- Defer seed dispersal, explicit microbial biomass and detailed plant organs from the
  first implementation. Background decomposition can represent an implicit community.

## Disagreements that remain real

**Food identity cannot be replaced by energy density.** Fable proposes retaining
litter/carrion distinctions through `rho = De/D`. That ratio contains no record of
origin or digestibility. Different substrates can have equal energy density; mixing
them destroys information needed to apply different diets. If we want a flesh eater
unable to live on leaves, preserve composition through separate stocks or equivalent
bookkeeping. This adds an ecological distinction, not necessarily a complicated
organ simulation.

**Boom control and niche design are separate questions.** Changing intake shape,
initial stocks and reproductive timing may improve population dynamics. It cannot
establish that animals have distinct dependencies on food sources. Two short runs
on stock and refuge cannot settle whether substrate identity is needed.

**Several purported agreements misstate Codex's position.** The original proposal
does not require a concave curve, forbid fixed assimilation coefficients, or demand
a permanently stable two-level community before predator access. Assimilation of
one food and ecosystem-wide trophic transfer efficiency are different quantities.
Nor did it prescribe removing every legacy depth preference. Choose habitat use
and feeding capabilities explicitly rather than treating these as settled agreements.

## Recommended minimal contract

1. Preserve living foliage, fruit, plant litter, animal tissue/carrion and mineral
   nutrients as distinct ecological resources. Living prey remains an organism;
   carcass representation must retain the relevant food identity.
2. Specify which foods each inherited capability profile can digest and its effective
   intake/assimilation and shared capacity. A compact table and a single explicit
   trade-off may suffice. Detailed mouth and gut modules are optional.
3. Include a minimal structural plant model in the first stage: edible foliage,
   persistent structure unavailable to ordinary grazers, and a finite budget for
   recovery. Defoliation reduces photosynthetic income; regrowth spends stored
   resources and/or new production. Continued damage and maintenance can exhaust
   that budget and kill the plant. Dead structure retains its identity as slowly
   decomposing woody material, not immediately edible leaf litter. Detailed roots,
   branches and organ geometry remain deferred. A type III intake curve may describe
   difficult harvesting, but does not replace these distinctions.
4. Tie maintenance, maturation and paid reproduction to accessible food over time.
   Use neither desired population counts nor guild percentages as ecological laws.
5. Keep local decomposition, finite energy losses and substrate transport explicit.
   Establish baseline flows before changing locality or decomposition rates.

This combines Fable's small-model discipline with the resource distinctions needed
for Wrysk's requested niches. It is a recommendation for review, not an agreed
implementation specification.

### Owner clarification after the comparison

Wrysk explicitly favored inedible or difficult-to-digest plant parts, including the
slow decomposition of wood, while retaining incremental development. The revised
recommendation above keeps that distinction in the first model and defers anatomical
detail. This records a design preference, not approval of specific equations or a
canon promotion. Preserved independent comparisons remain unchanged.

Structure alone does not explain or prevent continual defoliation. The first coupled
test must also compare realized leaf consumption with production, herbivore intake
limits and population growth. Success includes retained living foliage and recovery,
not merely persistent bare trunks. Test grazing, recovery, repeated stripping and
death with paid controls before asking a trained population to demonstrate them.

## Bounded next assignment to prepare

Draft that contract with a diet-by-resource table, conversion accounting, plant
recovery semantics and expected visible consequences. Resolve the equations for
capability trade-offs before naming their curvature or selecting numerical values.

Then specify a measured baseline plus individually bounded tests: initial-litter
sensitivity, paid reproduction versus recovery, and specialist/generalist competition
under reliable versus varying food. Preserve material and disclose energy changes
when altering initial food quality. Interpret a food-removal test as testing dependence,
not as proof the deprived niche should survive without its food.

Measure realized diets and net energetic viability, not only a multimodal genotype
histogram; the initial founders already differ. Later test coupled recovery beyond
the initial subsidy, predation and reproduction. Stop between milestones. No new
experiments, implementation or live changes were authorized or performed here.
