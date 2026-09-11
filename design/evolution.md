---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# Inherited bodies and habits

Use a compact, bounded genotype with three parts: physiological allocations,
a small procedural body grammar, and a fixed-size recurrent controller. Every
offspring receives a mutated copy. No evaluation run, elite population,
generation barrier, or scalar fitness function exists in the live world.

The genotype limits the search space deliberately. Novelty means new viable
combinations and interactions within that space; unrestricted invention of
new organs or programs is not part of the first system.

## Physiology and appearance share a phenotype

Decode genes once at birth into valid physical and behavioral parameters.
Allocations use normalized budgets with concave returns, rather than independent
sliders that can all reach maximum. Baseline maintenance depends on body mass,
maximum capabilities, and sensor/controller activity. Cheap inactivity must not
make expensive unused organs free.

| Inherited traits | Tradeoff or consequence | Readable expression |
| --- | --- | --- |
| Structural size and aspect, 1–4 linked lobes | Capacity, construction cost, turning drag | Dot, bead-chain, broad pod, hooked body |
| Fan area and pigment investment | Photosynthesis versus movement drag/exposure | Frill or thin fan; stable pigment hue |
| Mouth and digestion allocation | Grazing/scavenging/flesh access versus other organs | Blunt pad, split mouth, forward spike |
| Limb/tail investment and gait | Speed/turning/adhesion versus maintenance | Skitter, glide, pulse-step, curl-and-release |
| Armor and waste tolerance | Damage resistance versus growth and upkeep | Rim pixels, thick shell, slower cadence |
| Sensing radius and angular spread | Better information versus energetic cost | Antennae or directional feelers |
| Preferred light/moisture and metabolic rate | Habitat-dependent efficiency | Resting rhythm and migration response |
| Reproduction reserve, offspring size, dormancy | Many fragile young versus fewer provisioned young | Visible budding and size progression |
| Senescence onset and repair allocation | Longer life versus maintenance/reproduction | Gradual narrowing of bright core and slower recovery |
| Signal emission/response and chemical tag | Costly coordination, eavesdropping, exploitation | Rare pulse, short trail, inherited hue accent |

Initial body bounds: most adults cover roughly 3–7 pixels end-to-end, with an
absolute 9-pixel extent and small juvenile forms. Structural length, mass, food
handling, collision envelope, drag, and the renderer all derive from the same
decoded body. A cosmetic appendage must not claim a functional benefit that
does not exist. Decorative hue variation is allowed as a small inherited accent.

Use a bounded family of connected shapes, not a fixed catalog of species
sprites. Limb count, lobe offset, aspect, symmetry, and gait parameters can
combine. Mutations should preserve connected silhouettes. The core/mouth
direction stays readable even when a body is asymmetric or folded over an edge.

## Controller proposal

Start with approximately 24 normalized observations, 8 leaky recurrent units,
and 8 output channels. This is a working complexity budget, to benchmark before
freezing a schema. The dense controller has 320 weights plus biases and time
constants, followed by separate sensory/body genes, not an expanding neural genome.

Observation groups:

- Own reserve, age/repair state, recent damage, local crowding, and gait phase.
- Body-relative left/front/right food and suitability probes; estimates are
  filtered by inherited sensing range and food-detection investments.
- Relative bearing/approach of nearby visible bodies, weighted by size and
  chemical recognition; no perfect knowledge of another genome or energy store.
- Local light, moisture, waste, rim proximity, and signal gradients.

Exact channel packing is an M3 interface task. Keep a stable versioned channel
map in the eventual genome schema so mutations and checkpoints retain meaning.
Recurrent state supplies short memory, hysteresis, and oscillations. Sensor
noise, if present, uses a reproducible organism stream; it is never draw-frame
noise. Intrinsic phase and turn/throttle outputs drive embodied gaits.

Outputs request turn, movement effort, feeding effort, attack effort, defensive
posture, signal effort, reproductive investment, and dormancy/rest investment.
They pass through physiological limits and one explicit energy allocation step.
Conflicting requests share a finite budget. Empty feeding, attacking, and
signaling still incur bounded effort costs. A controller cannot command energy,
teleport, name another organism, bypass a seam, or create a child directly.

Retain small invariant protections: physiology rejects impossible actions;
starvation consumes reserve and eventually kills; range gates contact; numerical
rim reflection prevents leaving the surface. Founder controllers may have
hand-designed weights for viable feeding and movement. Those weights are
mutable and do not establish permanent behavioral classes. No within-lifetime
learning is proposed initially; inherited controller changes provide evolution.

## Birth, mutation, and lineage

Use asynchronous local asexual budding first. Sexual recombination is a later
possibility, not necessary for diversity and costly to make viable when tiny
populations split across niches. A parent must hold sufficient energy and
material, survive a gestation interval, and find local surface space. Escrow
offspring reserves during gestation; failed or interrupted gestation has an
explicit refund/detritus outcome. Capacity rejection occurs before a new escrow.

Offspring begin small with a bounded reserve, not a free full adult. Child
placement uses the same seam-aware geometry as movement. The parent remains
alive and pays the cost. Senescence reduces repair efficiency after an inherited
age; lifespans are not synchronized timers shared by a species.

Mutate mostly with small perturbations to physiology and controller weights.
Use rare bounded structural edits (add/remove a lobe or appendage, adjust
symmetry), plus occasional larger weight changes. Keep all genome magnitudes,
organ counts, controller time constants, and mutation rates bounded. Mutation
may reflect environmental stress modestly, but retain a baseline and a narrow
range; zero mutation and catastrophic mutation cascades are avoidable traps.

Development projects allocations into valid budgets and clamps physical ranges.
It does not silently replace an unviable child with a viable template. Physiological
viability is established by living, feeding, and surviving; no hidden fitness
gate filters offspring. Invalid numeric data is an implementation error and is
handled separately from a valid but unsuccessful mutation.

Maintain IDs, parent IDs, birth tick, genome digest, mutation summary, and origin
(descendant, dormant, recovered, founder). Full genealogies stream to bounded
archives rather than accumulating in RAM. Diagnostic species/strategy clusters
are descriptive and may change with the analysis method. The simulation does
not use their labels to assign food, mates, protection, or quotas.

## Showing that evolution matters

Track inherited trait distributions and behavioral samples in an external
observer. Capture an ancestor and descendants under the same neutral conditions
to distinguish inherited differences from momentary hunger or weather. Test
mutations in morphology, gait, sensing, metabolism, and controller response
independently before combining them.

Compare frozen genomes with mutable genomes over matched initial conditions,
recording both ecological diversity and visual differences. Test static weather
as well as moving weather. Report lineage age, trait variance, diet occupancy,
birth/death rates, and recovery counts without collapsing them into a fitness
number. Several colorful founder types surviving unchanged is not evidence of
evolution. Rapid color drift without changes in body or habits is also insufficient.

The largest risk is a controller that is technically evolvable but either too
fragile to reproduce or too opaque for changes to read at this resolution. If
that happens, reduce its dimensions or introduce structured sensory-to-action
biases before increasing network size. Preserve locality and mutable behavior.
