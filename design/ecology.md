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
hypotheses to validate, not guarantees of sustained diversity. After the
[review](7_Research/plan-review-2026-09-11.md), the starting ecology is one
producer pool, grazing, scavenging, and local recycling. Additional interactions
have separate experimental gates rather than arriving as a single package.

## Material and usable energy

Track material and usable energy separately. Energy supports actions and leaves
the model as metabolic heat. Material is tied up in bodies, reserves, food,
detritus, and dormant propagules; it limits how much living structure can exist.

| State | Ecological role | Visible consequence |
| --- | --- | --- |
| Free nutrient `N` | Shared mineral material for producers and growth | Indirect: fertile patches brighten as producers grow |
| Producer pool `P0` | Attached food requiring light and nutrient | Restrained substrate density/brightness |
| Detritus `D` and its stored chemical energy | Carrion, waste material, uneaten remains | Dimming flecks, later scavenger activity |
| Moisture and light | Spatial suitability and external energy supply | Slow changes in patch growth and behavior |

Producer biomass represents microbial background, not an immutable creature
species. A second pool `P1`, with different light/moisture response and recovery,
is an M4 experiment if spatial gradients and diet tradeoffs prove insufficient.
It competes for the same local nutrient, substrate area, and incident energy;
it must not double productivity merely by being enabled. Do not depend on
subtle pool colors being distinguishable at low LED brightness.

Waste/stress `W` and one costly signal channel are separate optional experiments.
There are zero chemical signal channels initially. Decorative history trails
are not sensed and do not alter resources. Shared waste clearance requires a
useful waste mechanism first; no benefit is awarded merely for forming a group.

The accounting invariant in a closed-material test is:

`M = sum(N + P0 + D + organism material)` initially; add `P1` and dormant
material only when those mechanisms are enabled.

Each term uses the same material unit. Organism material includes structural
mass and all stored organic reserves; `D` includes all loose organic material.
Maintain an explicit conversion table before implementing reactions. If an
interaction crosses cells or organisms, collect requests then settle transfers
from a shared pre-transfer state. Proportional allocation prevents iteration
order from awarding one consumer an entire patch by accident.

Proposed reaction rules:

- Producer photosynthesis spends available local light energy to convert `N`
  into organic reserves. It consumes nutrient, has bounded
  surface/capacity, and cannot turn ordinary light into material.
- Grazing, scavenging, and predation transfer finite food material and stored
  chemical energy. Assimilation inefficiency becomes detritus and heat, not
  disappearance or free reproduction credit.
- Maintenance, movement, sensing, signals, and defense consume stored chemical
  energy. Oxidized reserve material returns to `N`. If the later stress
  experiment is enabled, `W` rises with metabolic activity and is not counted
  a second time as material.
- Growth and budding transfer stored material into structure and pay construction
  energy. Death converts remaining structure/reserves to detritus, with bounded
  retained chemical energy. Decomposition returns `D` to `N` while dissipating
  its remaining energy. Carcasses need not be permanent entities.
- If enabled, signal concentrations are bounded informational fields paid for by energy;
  decay destroys the signal, not conserved nutrient.

Light, explicit environmental deposits, and rare recovery events are named
external sources. Respiration and inefficiency are named energy sinks. In closed
tests without light or injected energy, usable energy must not increase. Do not
repair negative resources by adding epsilon food; use bounded requests and
conservative numerical updates. Track any numerical residual separately.

## Organisms and interactions

All organisms share one life cycle and the [evolvable representation](evolution.md).
Allocation across grazing and scavenging creates the initial strategies. Add
facultative flesh digestion in M3c after the grazer tradeoffs pass E4. Any
organism can invest in it; none receives it free. A specialist hunter is a
possible episodic outcome, not a guild guaranteed permanent survival. Start
fixed-genome predation trials before mutating attack and avoidance together.

The minimal grazer is a primary failure hypothesis. Specify intake requests as
`q_i = min(k * mouth_i * effort_i * dt, remaining_gut_i)` with bounded,
appropriately costed mouth capacity. When a cell has Q available and requests
sum above Q, each consumer receives `Q * q_i / sum(q)`. Thus invested intake
capacity affects contested food share as well as isolated feeding rate, without
an arbitrary largest-body winner. Structural size limits mouth and reserve
capacity, and carries upkeep/drag; sensing costs energy and can help find less
depleted patches. These tradeoffs still do not guarantee size polymorphism.
E4 must expose whether smaller/cheaper always wins across tested conditions.

Organism photosynthesis is absent through M3. If later enabled, test lower
conversion efficiency than substrate producers, paid area/upkeep and drag,
local shading/space competition, and fan tissue edible by grazers. Allocate
one incident-light budget across producers and exposed fans. An efficiency
disadvantage alone cannot rule out static autotroph dominance; compare matched
runs for occupation, grazer survival, and motion before keeping this extension.

Contact requests are range-limited on the surface. Feeding and attack consume
handling time and energy. Attack outcome depends on size, mouth investment,
defense, approach, and the prey's current state; a predator gets at most the
material actually removed. Deduplicate target IDs across seams. Resolve
simultaneous predation without letting two attackers harvest the same body.

Later local signaling and heading alignment may allow trails, aggregations, and group
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
Include a gentle height gradient from embedded Y: upper surface brighter/drier,
lower surface dimmer/wetter, with horizontal patch variation and viable overlap.
Height is continuous across seams. This creates differing opportunities; it
does not by itself prove that population cycles will become asynchronous.

Overlay slow, bounded light and moisture variation: moving broad patches and
several phases with different periods, blended smoothly with low-rate stochastic
changes. Avoid a single global season that simultaneously favors the same
genotype everywhere. Do not roll new random weather every frame. Local grazing,
scavenging, and recycling should produce change even with weather held fixed.

Start ordinary movement in a roughly 0.5–2 pixels/second range, with bounded
paid bursts only if needed. E2 sweeps speed, patch spacing/recovery, and nutrient
diffusion together. Record travel time relative to patch recovery time and
field correlation length; slow diffusion alone is not a recipe for niches.

Use **100–200 active organisms cube-wide** as an initial visual/ecological
tuning hypothesis, with strong local variation. This is not an enforced count
or a 20–40-per-face quota. Tune fixed productivity and costs between runs;
never use a live controller that spawns or kills to reach the range. The 512
cap is a safety limit. Sparse periods and extinctions remain allowed.

Measure population coefficient of variation, lags/cross-correlation between
face and spatial-patch populations, time near the cap, empty-area duration,
birth/death flow, and substrate correlation length. Account for season/trends
and low counts when interpreting correlation; face totals alone can hide local
asynchrony. E2 tests organism-driven coupling under static and moving weather.

Starting real-time scales, for testing rather than promises:

| Time | Processes |
| --- | --- |
| 0.2–5 seconds | Steering, feeding, contact, escape; signals only if enabled |
| 5–60 seconds | Trails fade, grazing fronts move, local gatherings disperse |
| 2–15 minutes | Producer recovery, budding, local depletion, detritus succession |
| 10–90 minutes | Candidate window for local lineage/ecological turnover; generation time must be measured |
| 1–6 hours | Broad suitability shifts, migration, dormant recruitment |
| Days | Diverged descendants, patch memory, contingent ecological history |

These scales overlap. No event scheduler forces a bloom at minute ten or kills
a species at a fixed hour. Lifespan and reproduction timing evolve within
bounded ranges; some lineages should bridge several environmental phases.
E3 records parent ages at birth, first-reproduction times, and ancestry depth.
Do not infer effective population size or generations/day from live count and
the broad experience time-scale table.

## Why a successful strategy might stop being successful

| Feedback | Opportunity it creates | How it might fail |
| --- | --- | --- |
| Local food depletion and recovery | Moving fronts, resting, patch switching | Diffusion too fast erases patches |
| Height/patch gradients and limited specialization | Simultaneous habitats and feeding opportunities | Movement/diffusion couple the whole cube into one crash cycle |
| Predator dependence and prey defense costs | Coevolving pursuit, hiding, speed, armor | Hunters die before getting descendants |
| Later waste/habitat modification | Abandonment and later recolonization | Stress becomes an indiscriminate extinction switch |
| Costly sensing; later signaling | Ambush, exploitation, social and solitary strategies | Signals become visual decoration only |
| Slow asynchronous forcing | Regional succession and migration | Weather produces all the apparent variety |
| Paid dormancy | Return of previously unfavored descendants | Dormancy hides an otherwise nonviable active ecology |

There is no population-wide fitness ranking, generation replacement, automatic
reward for rarity, or objective that the organisms optimize collectively. Local
survival and paid reproduction create selection. Diagnostics observe diversity;
they do not quietly adjust fitness or kill common lineages.

## Continuity, dormancy, and rare recovery

First seed a heterogeneous world with a handful of visibly different founder
strategies and viable producer patches. Founders are starting genomes, not
protected classes, fixed population quotas, or repeatedly respawned champions.

In the M4 dormancy experiment, organisms may invest material and energy in local
dormant propagules. Each
contains an actual descendant genome and a finite reserve. Storage has a hard
global cap (initially 512); propagules decay and germinate only when their
environmental thresholds and available space allow. A full bank rejects new
deposits without charging the parent. Evicted or expired material returns to
detritus; no species receives guaranteed archive space.

Keep a bounded historical genome library (initially 128 entries) in the observer,
sampled across births/time without fitness or lineage quotas. It is an archive
for comparisons, not living biomass, a recruitment source, or core simulation
state. The observer cannot reintroduce a genome into the running world.

There is one exogenous fallback: **logged reseeding after complete mobile
extinction**, when no viable dormant propagules remain. Debounce the extinction
condition, then place one bounded founder cohort under a cooldown and explicit
material/energy accounting. Debit available nutrient and label founding energy
as an external source. If required material is unavailable, wait for recycling;
do not create unaccounted mass. Data corruption is a separate operational fault.

The extinction test counts all living nondormant creatures, including stationary
phenotypes. Low movement does not mean extinction; substrate producer pools are
not creatures for this test. This distinction matters if autotrophs are later added.

Persist the trigger/cooldown and log population, cause, cohort, and resource
budget. There is no below-eight recruitment, historical-library recruitment,
periodic immigrant rain, or diversity/activity-triggered rescue. Dominance and
a sparse but viable world do not trigger intervention. Long waits for dormant
germination must be visible to diagnostics and reviewed for ambient quality.

Evaluate four conditions where dormancy is implemented: neither dormancy nor
reseeding; dormancy only; reseeding only; both. Dormancy is ecological storage
but may still be responsible for persistence. Tag dormant versus exogenous
recruitment separately. Frequent resets fail the unattended ecology gate even
if they keep the display occupied. All survival claims state which mechanisms
were enabled; no fallback is enabled during the initial feeding/evolution gates.
