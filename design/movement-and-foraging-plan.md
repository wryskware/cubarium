---
design_status: exploration
last_reviewed: 2026-09-14
---

# Bodies, food patches, and purposeful activity

Proposal for Wrysk's review. This document implements no behavior and promotes no
ledger decision. Implementation/delegation follows review; M1 search changes and
sustainability tuning follow the behavioral redesign. Updated after Wrysk clarified
that pivoting is allowed and selected recurrent control without an MLP comparison.
The detailed architecture and implementation milestones remain proposals for review.

The owner's direction is that organisms have bodies that occupy space, must not
spin faster than they can move, and should visibly do things rather than wiggle
around a renewable food supply. The design should produce meaningful activity
through physical movement and ecological need.

**Proposed experience:** an animal travels to a food patch, settles to eat,
visibly reduces the food, and leaves when the patch can no longer support it.
Another glance shows it somewhere else. A depleted patch stays depleted long
enough for that journey to matter. Fed animals can genuinely rest. Predators
search, approach, capture, handle meals, and relocate when hunting fails.

## 1. Make movement belong to the whole body

Replace independently chosen translation and heading changes with one embodied
locomotion rule shared by ordinary fauna and apex, including pursuit and escape.

- Measure movement in body lengths and turning in the distance swept by the
  body's outer extent. A larger body turns more slowly for the same available
  locomotion speed.
- Pivoting in place is allowed. The center may stay fixed while the outer body
  travels around it. This is locomotion with a speed limit and energy cost;
  actual resting means neither translating nor pivoting.
- Linear and angular acceleration are gradual. There is no fixed steering
  radius, mandatory forward departure, or car-like constraint.
- Water, low available energy, and other locomotion limits reduce both the
  ability to travel and the ability to turn. Turning has an energetic cost.
- Hunting, fleeing, mating, crowd avoidance, and presentation must all respect
  the same rule. A predator cannot instantly rotate its entire body to aim a
  distant claw at prey.

A candidate mathematical contract is
`|linear speed| + r × |angular speed| ≤ available locomotion speed`, where `r`
measures the relevant body's extent from its pivot. A pure pivot spends the
budget on rotation; a straight movement spends it on translation. The bound is
against available movement capability, not current center displacement. This is
a conservative stylized movement budget, not a full mechanics model; exact
integration, footprint clearance and costs belong to implementation.

Use a footprint appropriate to the actual body and growth stage. Decorative
feelers need not be solid obstacles, but a long apex cannot turn as if it were a
point. Nearby bodies should cause an approaching animal to slow or take a wider
route before overlap, rather than endlessly rotate against a repulsion vector.
Use the existing surface/shim geometry contract for continuous travel and body
orientation across cube seams. A chart change is not a physical turn.

Do not build a general rigid-body engine. The scope is coherent travel, bounded
turning, and useful space between animals. Flight, swimming and crawling can
have different locomotion capabilities within that common contract.

## 2. Make feeding consume a place

Treat food as a finite accessible stock with a recovery process. An animal's
mouth/body reach determines where it can eat. Merely remaining anywhere in a
coarse simulation cell must not provide access to untouched food indefinitely.
The underlying grid may remain coarse if its accessible stock and presentation
make depletion spatially honest; increasing resolution is not itself the goal.

For a viable feeding animal in a productive patch, consumption during a meal
should exceed local renewal. It can take a useful meal, but cannot harvest that
same spot forever. Depletion should outlast the feeding visit and the journey
to another patch. An undisturbed patch should recover over several such visits,
with recovery limited by light, water, nutrients, and remaining producer tissue.

This requires changing the relative scales of food stock, intake, maintenance,
movement and regrowth together. The direction is slower local replenishment
relative to feeding, with enough standing food to make arrival worthwhile.
Reducing global growth alone could leave the current animals starving in place.

Proposed producer model: retain living/root tissue that can rebuild edible
tissue, with a clear distinction between what survives grazing and what an
animal can currently eat. First check whether the existing producer stock and
grazing refuge can express this faithfully; introduce a separate edible stock
only if needed. There must be a resource-funded route to patch recovery without
an inexhaustible edible refuge or automatic food refill.

Scavengers follow the same principle: litter and carcasses are finite deposits
that lose material and usable energy. They must move as deposits are consumed.
Predators consume actual prey and have finite meal stores. No feeding channel
gets a local resource subsidy to keep an idle animal alive.

Vegetation and meal presentation must show these stocks. A grazed patch should
look grazed and regrow on the ecological recovery schedule. Existing gradual
plant transitions can smooth the change without hiding depletion.

## 3. Give animals sensory causes for behavior

Searching, approaching, feeding, leaving and resting describe things an observer
should see; they need not be explicit states or destinations in every animal's
controller. Following a local food gradient, responding to changing concentration
over time, or maintaining a course can all produce useful foraging. A hunter may
track an individual prey while another organism never represents a target.

Local conditions and internal needs should be enough to distinguish a useful
feeding opportunity from an exhausted one. Supply local food information, actual
recent intake, reserves, movement feedback and nearby interactions. Provide a
small memory capacity so behavior can depend on whether conditions are improving,
rather than prescribing a global map or a particular patch-memory algorithm.

The desired outcomes remain coherent exploration, departure from unproductive
food, escape from danger, and quiet after sufficient intake. They are behavioral
evaluation cases, not a mandatory sequence hardcoded into every organism.

## 3a. Recurrent control from the outset

Wrysk has selected recurrent control directly, without building an MLP comparison.
The detailed [recurrent-organism plan](recurrent-organism-plan.md) develops that
choice from research into a GRU architecture, local senses, motor actions,
training, inheritance and bounded delivery milestones. Its specific engineering
choices remain proposals for review.

Establish the body's physical rules and finite food supply, then train the
recurrent controller before expanding the hand-authored behavior system. Keep
existing control only as an integration reference. Each animal has private
memory and inherited weights; the world enforces costs, contact and resource
limits. Searching, feeding, leaving and hunting describe outcomes to evaluate,
not compulsory named controller states.

The detailed plan recommends a small GRU, a bounded offline neuroevolution pilot,
and several viable policies followed by heritable variation. It separates memory
from lifetime weight learning and places M1 sustainability tuning after the
behavioral model is established. No training or delegation has started.

## 4. Give the apex a complete hunting journey

The following are desired observable capabilities, not a required hand-authored
phase machine for decision making. Physical attack/handling phases still have
world-enforced timing and costs.

An apex may wait briefly where an encounter is plausible, and remain quiet while
handling a meal. An unsuccessful hungry apex must eventually relocate.

- No nearby prey: search through suitable habitat, with a persistent course.
- Eligible prey sensed: choose a target, approach and align the body through real
  motion, then attempt capture only when the effector can physically reach it.
- Prey lost or repeated approach failure: abandon and relocate rather than
  circle the same position indefinitely.
- Capture: handle and digest a finite meal; rest when sufficiently fed.
- Renewed hunger: resume searching. A depleted hunting area should empty of
  predators as well as prey.

Mate encounters also depend on travel and local sensing; reproduction must not
become the new reason for indefinite rotation or waiting. Existing paid births,
dormancy and encounters remain part of the ecology, subject to later balance.

## 5. Let population follow available food and space

Do not use the numerical population cap as the intended carrying capacity.
Crowded patches divide finite food among visitors and become unprofitable
sooner. Competition leads to dispersal, reduced reproduction, and deaths when
food is insufficient. Reproduction spends surplus after living and movement
costs; offspring create additional demand and must find feeding opportunities.

Start development demonstrations with a sparse, varied cohort so individual
journeys are readable. An initially smaller population is a viewing condition,
not evidence that density has been solved. If it rapidly grows into a crowded
static carpet, the design has failed this objective.

The intended relationship is: a feeding visit depletes a local patch; nearby
patches can support continued foraging; recovery occurs while animals are away;
total production limits the number of animals sustained over time. Exact
abundance, recovery times and reproductive rates are later tuning questions.

## Delivery sequence after review

1. **Body, food, and controller boundary.** Establish pivot-capable movement with
   shared speed/energy limits, finite reachable food and slower patch recovery.
   Define local observations and bounded actions. Use the existing controller
   with minimal adaptation to exercise these rules; do not build the full new
   heuristic behavior system. Demonstrate turns, pivots, contact, seam travel,
   consumption and recovery independently of controller intelligence.
2. **Bounded neural foraging pilot.** After a separate measured compute checkpoint,
   train a small policy to forage in the real core. Compare against the baseline
   with the same senses, actions and world conditions. Demonstrate useful intake,
   depletion, departure and feeding elsewhere on held-out situations. Stop and
   review before expanding training or adding mechanisms.
3. **The complete food web.** Extend the successful controller approach across
   diets and apex, crowding, escape and reproductive demand. Preserve variation
   between individuals/lineages. Observe the ordinary mixed world on the cube;
   passing an isolated foraging task is not sufficient evidence of good ecology.
4. **Behavior review, then sustainability search.** Once the behavior is accepted,
   update M1's evaluation and search scope. Tune ecological parameters across
   seeds and longer horizons without using easier food to conceal controller
   failure. Keep offline policy optimization and ecological balance objectives
   explicit, even if a later experiment couples them.

Each implementation milestone gets a fresh bounded handoff, focused checks and
at most one targeted review, with a spending checkpoint before the next. Do not
launch the whole sequence as an open-ended agent campaign. Use the normal build
cache and current development checkout; arrange display updates under the
working policy when changes are ready. This proposal authorizes no reset.

## What counts as success

At native size, the viewer can recognize an animal traveling somewhere, feeding,
leaving and later doing something elsewhere. Rest is clearly still. Pivots move
the outer body at a plausible, paid speed. A predator approaches prey or relocates after
failure. Resource depletion and recovery explain why those journeys happen.

Focused checks should cover the movement bound, seam continuity, finite shared
food, conservation, departure after failed intake, and recovery on an abandoned
patch. Compare time spent traveling with displacement in body lengths, fresh
ground reached, patch residence and actual intake; a tight circle cannot count
as successful exploration just because its path is long. These are development
observations, not analytical overlays on the normal cube.

No sustainability claim follows from these demonstrations. The deliverable is a
coherent, readable ecological behavior model with explicit parameters. M1 can
then help find sustainable settings for that model.

## Basis and limits

This proposal responds to Wrysk's 2026-09-14 correction and the preceding source
diagnosis. Current implementation anchors are ordinary steering and effort in
`crates/cubarium-core/src/controller.rs:154–250`, apex heading/effort overrides in
`crates/cubarium-core/src/world.rs:1595–1641`, shared locomotion in
`world.rs:1682–1724`, resource reactions in `fields.rs:102–195`, and proportional
feeding settlement in `world.rs:1940–2020`. Those mechanisms establish the
starting point; none makes the proposed replacements canonical. The diagnosis
did not establish an exact local food-renewal-to-consumption ratio, and this
plan does not claim one.
