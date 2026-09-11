---
design_status: decided
last_reviewed: 2026-09-11
decision_refs:
  - D-0002
---

# Owner's brief

Source: Wrysk's task instruction, 2026-09-11. This is a faithful requirements
summary, not a verbatim transcript. Mechanism choices are in separate proposals.

## Purpose and experience

Create a persistent decorative artificial ecosystem for an office and parties.
It should feel like a tiny living world on an object: organisms wander, feed,
interact, reproduce, change, compete, cooperate, cluster, and migrate. Unplanned
behavior and long-term surprise matter more than biological realism.

It should run for hours, days, or longer without restarting experiments. This
is not principally a benchmark, scientific evolution experiment, game, or
conventional genetic algorithm maximizing a fitness number. Avoid an inevitable
single dominant optimum. Support diversity, niches, changing pressure,
coevolution, succession, population cycles, and local adaptation. Extinction,
temporary dominance, and dramatic shifts are welcome; permanent stagnation
should be unlikely.

The default image is only the world: no labels, graphs, menus, scores, or debug
overlays. Vary intensity between quiet periods, local activity, blooms,
migrations, predator events, and emergent formations. Constant visual noise or
arcade-like motion would miss the intended ambient character.

## Physical world and organisms

Five visible faces, each 64×64: Front, Left, Back, Right, and Top. These are
connected surfaces of a physical cube. All spatial phenomena, including motion,
orientation, features, trails, fields, and particles, should respect seams.
Use a unified surface coordinate model rather than five independent simulations.

The existing shim is at `~/vuzic/led-cube-shim`; do not rebuild hardware or
display transport. The brief lists faces, not protocol indices; the shim's
actual ordering and orientation are recorded in the integration findings.

Embrace low resolution. Organisms should remain legible through silhouettes,
motion, color, appendages, trails, and pulses. Prefer genes with observable
consequences. Evolution may affect morphology, locomotion, sensing, metabolism,
reproduction, lifespan, aggression, avoidance, social tendencies, predation,
environmental preference, signaling, and appearance. This is a palette of
possibilities, not an obligation to implement every listed trait.

The environment has its own dynamics and offers niches. Activity should span
seconds (individual actions), minutes (local ecology), hours (population and
ecosystem changes), and longer periods (meaningful evolution).

## Future interaction

Provide clean extension points for audio features or analysis, vision,
proximity, motion, face-local touch or gestures, and later sensors. Prefer
environmental forces and stimuli to direct creature commands: humans perturb
the ecosystem. Implementing every input is outside the current request.

## Engineering priority order

1. Strong simulation architecture.
2. Seamless cube-surface topology.
3. Compelling emergent ecology.
4. Evolution that resists convergence and stagnation.
5. Readability at 64×64 per face.
6. Long-running stability and performance.
7. Clean environmental and interaction hooks.

Take ownership of mechanism selection and tradeoffs. The eventual implementation
should be a real system. **The immediate deliverable is repository initialization
and plans in the Lore design vault format**, not simulation implementation.
