---
design_status: leaning
last_reviewed: 2026-09-11
decision_refs: []
---

# Inherited bodies and habits

Use a compact, bounded genotype with three parts: physiological allocations,
a small procedural body grammar, and named behavioral drives with short memory. Every
offspring receives a mutated copy. No evaluation run, elite population,
generation barrier, or scalar fitness function exists in the live world.

The genotype limits the search space deliberately. Novelty means new viable
combinations and interactions within that space; unrestricted invention of
new organs or programs is not part of the first system.

This revision follows F1, F2, and F9 of the
[plan review](7_Research/plan-review-2026-09-11.md). The
[response](7_Research/plan-review-response-2026-09-11.md) distinguishes adopted
recommendations from claims still requiring experiments.

## Physiology and appearance share a phenotype

Decode genes once at birth into valid physical and behavioral parameters.
Allocations use normalized budgets with concave returns, rather than independent
sliders that can all reach maximum. Baseline maintenance depends on body mass,
maximum capabilities, and sensor/controller activity. Cheap inactivity must not
make expensive unused organs free.

| M3 inherited traits | Tradeoff or consequence | Readable expression |
| --- | --- | --- |
| Structural size/aspect and 1–4 linked lobes | Capacity, construction cost, turning drag | Dot, pod, bead-chain |
| One head or tail appendage form/extent | Mouth handling or propulsion, with paid structure | Directional accent and turning style |
| Mouth and grazing/scavenging allocation | Maximum intake and contested-resource share versus upkeep | Feeding persistence and patch choice |
| Sensing range and metabolic rate | Information/speed versus energetic cost | Directed movement, search cadence, pauses |
| Reserve capacity, offspring provision, repair | Lean-period survival versus construction and reproduction | Growth, budding, resting cadence |
| Light/moisture preferences and behavioral gains | Local habitat efficiency and action costs | Migration, pursuit, escape, stop/start habits |
| Small inherited hue accent | Primarily cosmetic lineage resemblance | Stable accent, not a new color per birth |

M3 starts with three geometric controls: lobe count, body length/aspect, and one
head-or-tail appendage choice. Hue is a separate inherited accent; gait carries
much of the remaining variation. E6 tests twelve samples on the cube before
expanding the grammar. Functional anatomy still affects physiology, but every
parameter need not have a separate one-pixel ornament.

Light harvesting, fans, armor ornament, chemical tags, waste tolerance, signals,
and dormancy traits are later, independently tested extensions. No photosynthesis
exists in M3 organisms. Do not add a fake photosynthetic fan for appearance; a
decorative shape must be identified as such and cannot claim a missing function.

Initial body bounds: most adults cover roughly 3–7 pixels end-to-end, with an
absolute 9-pixel extent and small juvenile forms. Structural length, mass, food
handling, collision envelope, drag, and the renderer all derive from the same
decoded body. A cosmetic appendage must not claim a functional benefit that
does not exist. Decorative hue variation is allowed as a small inherited accent.

Use a bounded family of connected shapes, not a fixed catalog of species
sprites. Start with the limited controls above; additional asymmetry or limb
rules need visual evidence. Mutations should preserve connected silhouettes. The core/mouth
direction stays readable even when a body is asymmetric or folded over an edge.

## Controller proposal

Start with roughly 12–20 named drives/thresholds, plus two memory time constants.
Use food-gradient attraction, light/moisture preference, approach/avoidance by
relative body size, hunger-dependent search effort, rest threshold, feeding
persistence, flee-on-damage gain, turn persistence, gait period/amplitude, pause
duration, and budding threshold. Version the exact parameter map during M3a;
inactive mechanisms have no hidden controller inputs. Rim sensing, waste, and
signals are absent until their individual experiments justify them.

Observations are local scalar values and body-relative probes/bearings with
inherited sensing range. An organism cannot inspect another genome, lineage
label, or exact energy store. Start without sensor noise. Exploration can use
bounded persistent turns from a dedicated reproducible organism random stream.

Two leaky memories summarize recent hunger and damage. For normalized input x,
update `m += (1 - exp(-dt/tau)) * (x-m)` with inherited bounded positive tau.
These state variables permit delayed feeding/search transitions and prolonged
alarm. Combine weighted named steering drives, then limit turn rate and effort;
thresholds and hysteresis choose feeding, rest, and reproduction investment.
An oscillator supplies gait phase. Individual parameter effects are inspectable,
but combined trajectories need not be monotone or reducible to one drive.

Outputs request movement, feeding, rest, budding, and, when enabled in M3c,
attack effort. They pass through physiological limits and a finite energy
allocation step. Empty effort still costs energy. Physiology rejects impossible
actions, range gates contact, starvation eventually kills, and reflection keeps
organisms on the surface. A valid parameter set is not a viable organism: it
must still find food and reproduce. Founder parameter lists are hand-authored
starting points; the drives remain mutable, with no protected species scripts.

The earlier 24-input/8-unit/8-output dense recurrent network (320 weights before
biases and time constants) is a deferred candidate. E5 first measures how viable
and behaviorally varied sparse reflex mutations are. If a specific missing
behavior warrants a network, compare a bounded residual modulator under the
same mutation and resource budgets; do not add hundreds of dimensions merely
because reflexes are simpler. Neither representation guarantees readable
evolution. No within-lifetime learning is proposed initially.

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

Use sparse mutations: a configurable probability of mutation per birth, then
change a small bounded number of loci (initially 1–3), usually by a small step.
Many children may be exact copies. Rare structural edits alter one of the
enabled morphology controls. Mutations in normalized allocations can affect
several decoded traits; record those consequences, not just the raw changed locus.
Keep parameters and memory constants bounded. Fix the mutation policy per run;
stress-modulated or evolved mutation rates are deferred to preserve attribution.

Development projects allocations into valid budgets and clamps physical ranges.
It does not silently replace an unviable child with a viable template. Physiological
viability is established by living, feeding, and surviving; no hidden fitness
gate filters offspring. Invalid numeric data is an implementation error and is
handled separately from a valid but unsuccessful mutation.

Maintain IDs, parent IDs, birth tick, genome digest, mutation summary, and origin
(descendant, dormant, extinction-reseed, founder). Full genealogies stream to bounded
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

Run E4 before predation: mutate size, metabolism, sensing, and reserve capacity
under grazing/scavenging alone. Consistent collapse to all lower bounds is a
failure of the proposed tradeoffs, not a reason to protect large lineages by
quota. Measure time to first reproduction, parent age at each birth, and ancestry
depth (E3); births per hour alone is not a generation-time estimate. E5 measures
mutant viability and behavioral differences under matched habitats before M3b.
Experiments and stopping rules are in the [experiment plan](experiments.md).
