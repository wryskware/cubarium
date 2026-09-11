---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# An environment that remembers its inhabitants

The preferred model couples mobile organisms to a slowly changing microbial
substrate. Resources are local, consumption changes future opportunity, and
material recycles after death. The aim is several overlapping ways to live,
with feedback that makes prolonged dominance costly. These mechanisms are
hypotheses to validate, not guarantees of sustained diversity.

## Material and usable energy

Track material and usable energy separately. Energy supports actions and leaves
the model as metabolic heat. Material is tied up in bodies, reserves, food,
detritus, and dormant propagules; it limits how much living structure can exist.

| State | Ecological role | Visible consequence |
| --- | --- | --- |
| Free nutrient `N` | Shared mineral material for producers and growth | Indirect: fertile patches brighten as producers grow |
| Producer pools `P0`, `P1` | Attached food with different light/moisture optima | Two restrained substrate textures/colors |
| Detritus `D` and its stored chemical energy | Carrion, waste material, uneaten remains | Dimming flecks, later scavenger activity |
| Waste/stress `W` | A decaying, nonmaterial environmental modifier created by metabolism | Local desaturation or sparse discoloration |
| Moisture and light | Spatial suitability and external energy supply | Slow changes in patch growth and behavior |
| Two signal channels | Short-lived, costly, locally diffusing cues | Subtle trails or pulses when organisms emit |

Producer pools represent microbial background, not two immutable creature
species. `P0` favors brighter/drier conditions, `P1` dimmer/wetter conditions;
both require some light and nutrient. They compete for substrate capacity and
have different recovery rates. Initial parameter values remain tuning work.

The accounting invariant in a closed-material test is:

`M = sum(N + P0 + P1 + D + organism material + dormant material)`.

Each term uses the same material unit. Organism material includes structural
mass and all stored organic reserves; `D` includes all loose organic material.
Maintain an explicit conversion table before implementing reactions. If an
interaction crosses cells or organisms, collect requests then settle transfers
from a shared pre-transfer state. Proportional allocation prevents iteration
order from awarding one consumer an entire patch by accident.

Proposed reaction rules:

- Photosynthesis spends available local light energy to convert `N` into
  producer or organism organic reserves. It consumes nutrient, has bounded
  surface/capacity, and cannot turn ordinary light into material.
- Grazing, scavenging, and predation transfer finite food material and stored
  chemical energy. Assimilation inefficiency becomes detritus and heat, not
  disappearance or free reproduction credit.
- Maintenance, movement, sensing, signals, and defense consume stored chemical
  energy. Oxidized reserve material returns to `N`; temporary stress `W` rises
  with metabolic activity and is not counted a second time as material.
- Growth and budding transfer stored material into structure and pay construction
  energy. Death converts remaining structure/reserves to detritus, with bounded
  retained chemical energy. Decomposition returns `D` to `N` while dissipating
  its remaining energy. Carcasses need not be permanent entities.
- Signal concentrations are bounded informational fields paid for by energy;
  decay destroys the signal, not conserved nutrient.

Light, explicit environmental deposits, and rare recovery events are named
external sources. Respiration and inefficiency are named energy sinks. In closed
tests without light or injected energy, usable energy must not increase. Do not
repair negative resources by adding epsilon food; use bounded requests and
conservative numerical updates. Track any numerical residual separately.

## Organisms and interactions

All organisms share one life cycle and the [evolvable representation](evolution.md).
Allocation across light harvesting, grazing, scavenging, and flesh digestion
creates strategies rather than assigning rigid roles. Specialization yields a
benefit; spreading investment across all strategies limits each. A hunter still
needs food when prey is scarce. A frilled light harvester sacrifices speed and
pays for exposed area. An armored grazer spends more on structure and motion.

Contact requests are range-limited on the surface. Feeding and attack consume
handling time and energy. Attack outcome depends on size, mouth investment,
defense, approach, and the prey's current state; a predator gets at most the
material actually removed. Deduplicate target IDs across seams. Resolve
simultaneous predation without letting two attackers harvest the same body.

Local signaling and heading alignment allow trails, aggregations, and group
movement. Signals report concentration, not trustworthy facts. Emitters pay;
receivers can exploit signals without reciprocating. Avoid a hardcoded species
friend/enemy test. Heritable chemical tags can influence recognition, but cannot
be a perfect global lineage oracle.

Cooperation should have a physical benefit before adding a reward for it. For
example, local waste clearance investment can improve a shared patch, making
clusters viable while allowing noncontributors to exploit them. Add that
investment only after waste and signaling work independently. Crowding, resource
depletion, and disease-like stress from waste oppose unlimited clustering.

## Habitat and time scales

Generate a fixed, spatially smooth substrate from the embedded cube position:
porosity changes nutrient diffusion and water retention; roughness changes
locomotion cost. Keep these contrasts soft, with no impassable pixel maze. A
continuous 3D function restricted to the surface avoids face-specific islands.

Overlay slow, bounded light and moisture variation: moving broad patches and
several phases with different periods, blended smoothly with low-rate stochastic
changes. Avoid a single global season that simultaneously favors the same
genotype everywhere. Do not roll new random weather every frame. Local grazing,
waste, and recycling should produce change even with weather held fixed.

Starting real-time scales, for testing rather than promises:

| Time | Processes |
| --- | --- |
| 0.2–5 seconds | Steering, feeding, contact, escape, short signals |
| 5–60 seconds | Trails fade, grazing fronts move, local gatherings disperse |
| 2–15 minutes | Producer recovery, budding, local depletion, detritus succession |
| 10–90 minutes | Several organism generations, predator/prey feedback, colony turnover |
| 1–6 hours | Broad suitability shifts, migration, dormant recruitment |
| Days | Diverged descendants, patch memory, contingent ecological history |

These scales overlap. No event scheduler forces a bloom at minute ten or kills
a species at a fixed hour. Lifespan and reproduction timing evolve within
bounded ranges; some lineages should bridge several environmental phases.

## Why a successful strategy might stop being successful

| Feedback | Opportunity it creates | How it might fail |
| --- | --- | --- |
| Local food depletion and recovery | Moving fronts, resting, patch switching | Diffusion too fast erases patches |
| Different light/moisture optima and limited specialization | Simultaneous habitats for different diets | One resource dominates total productivity |
| Predator dependence and prey defense costs | Coevolving pursuit, hiding, speed, armor | Hunters die before getting descendants |
| Waste and habitat modification | Abandonment and later recolonization | Stress becomes an indiscriminate extinction switch |
| Costly sensing/signaling and limited attention | Ambush, exploitation, social and solitary strategies | Signals become visual decoration only |
| Slow asynchronous forcing | Regional succession and migration | Weather produces all the apparent variety |
| Dormancy and historical propagules | Return of previously unfavored descendants | Archive freezes change or creates free organisms |

There is no population-wide fitness ranking, generation replacement, automatic
reward for rarity, or objective that the organisms optimize collectively. Local
survival and paid reproduction create selection. Diagnostics observe diversity;
they do not quietly adjust fitness or kill common lineages.

## Continuity, dormancy, and rare recovery

First seed a heterogeneous world with a handful of visibly different founder
strategies and viable producer patches. Founders are starting genomes, not
protected classes, fixed population quotas, or repeatedly respawned champions.

Organisms may invest material and energy in local dormant propagules. Each
contains an actual descendant genome and a finite reserve. Storage has a hard
global cap (initially 512); propagules decay and germinate only when their
environmental thresholds and available space allow. A full bank rejects new
deposits without charging the parent. Evicted or expired material returns to
detritus; no species receives guaranteed archive space.

Keep a separate bounded historical genome library (initially 128 entries),
sampled across birth history and time by reservoir sampling, without fitness
or lineage quotas. It is metadata, not living biomass. A slow recovery path
may draw from it, but must explicitly obtain founding material and energy.

Recovery preference order:

1. Ordinary producer recovery, migration, recycling, and local germination.
2. When the active mobile population stays below a low threshold (initially
   8 for 10 simulated minutes), offer a small number of historical genomes at
   suitable local patches; debit real nutrient and an explicit bounded energy
   input. Use a cooldown, not repeated per-tick spawning.
3. After complete mobile extinction with no viable propagules, admit a small
   immigrant cohort from a versioned founder reservoir. Use the same budgets and
   log origin. If material itself is exhausted or corrupted, report that fault;
   do not hide it with this ecological mechanism.

For nonempty but prolonged inactivity, a rare local disturbance may follow a
conservative fixed cooldown (hours), never a diversity score. The first version
should rely on ordinary weather and dormancy for this case; only add an activity
guard if observed failures justify it. Dominance alone is not failure.

All assistance is tagged and counted in diagnostics. Accelerated evaluations
also run with assistance disabled. Frequent use means the ecology needs repair;
it must not be celebrated as self-sustaining change. The bounded recovery path
serves the unattended installation, while evaluations expose its contribution.
