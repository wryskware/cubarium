---
design_status: exploration
last_reviewed: 2026-09-15
decision_refs: []
---

# Reconsidering Cubarium's food web

Wrysk wants distinct diets, ecological niches and interdependence, reconsidered
independently of the current implementation. This is a proposal for review, not an
accepted contract or authorization to change the running world. The existing diet
slider, resource pools, founder kinds and milestone boundaries are revisable.

The desired outcome is a visibly living community: animals have different reasons
to inhabit and travel through the world; feeding changes their surroundings; those
changes create opportunities and limits for other organisms. A dense swarm surviving
on visually obscure food does not satisfy that outcome even if accounting closes.

## What the current model obscures

The legacy controller permits grazing at `diet >= 0.05`, scavenging at
`diet <= 0.95`, and fruit eating at `diet >= 0.5`
(`crates/cubarium-core/src/controller.rs:197`). The existing founder proposal assigns
burrowers 0.10, grazers 0.85, gliders 0.90 and skimmers 0.60
([fauna v2](fauna-v2.md)). All fall in the overlap between grazing and scavenging.
The mouth-rate allocation provides a trade-off, but a small fallback intake can
still matter if it covers maintenance. Names and appearance do not establish
ecological specialization.

In the inspected live run at tick 14,300, population had grown from 28 to 179 after
169 births and 18 starvation deaths, peaking at 184. Only four animals were neural
at the sampled status endpoint. Initial detritus energy was about 1,335 at tick 100
and about 202 remained at tick 14,300. This identifies a substantial initial food
stock, not a measured partition of consumption versus decomposition. It does not
establish immortality or eventual recovery. The source telemetry remains in the
active, changing `state/telemetry.jsonl`; this paragraph records the observation.

## Proposed food web

Use a few distinct resources whose differences change who can eat them and how.

| Resource | Proposed origin and fate | Main consumers |
| --- | --- | --- |
| Living foliage and soft shoots | Plant growth; browsing, senescence or plant death removes it | Herbivores |
| Fruit and seeds | Costly plant reproduction; seasonal or local availability; fruit may fall and decay | Fruit specialists and selected omnivores |
| Plant litter | Fallen leaves, dead stems and plant residues; progressively processed and depleted | Detritivores and microbial decomposers |
| Animal tissue and carrion | Living prey or a finite carcass after death | Predators and scavengers with suitable capabilities |
| Mineral nutrients | Released by metabolism/decomposition; required for new plant growth | Plants and microbes, not directly an animal meal |

Carcasses and old leaves can both be called detritus broadly, but merging them into
one universally edible food loses a useful distinction. Keep food origin, remaining
energy and digestibility meaningful. Exact pool representation can follow review.

Plants should also have meaningful internal allocation: browsing leaves reduces
photosynthetic and reproductive capacity; it need not instantly erase every trunk
and root. Stored resources can support recovery, at a cost. Repeated stripping can
kill a plant. Protected structure, recovery and seed establishment should create
observable history rather than a field instantly refilling after each bite.

## Bodies establish diets; brains choose behavior

Give organisms inherited feeding and digestive capabilities. A plant-litter
specialist can maintain itself on food a leaf specialist cannot digest usefully.
A predator may scavenge flesh without being able to live on rotten leaves. Limited
omnivory is allowed where a body actually supports it.

Use a shared investment budget for mouth structures, gut capacity and digestive
machinery, with maintenance and handling costs. Broad diets should sacrifice some
combination of throughput, assimilation, speed or reproductive investment. Do not
multiply arbitrary penalties: select the smallest mechanism that produces a
measurable trade-off. A specialist should have an advantage on its own resource;
a generalist should have an advantage when that resource becomes unreliable.
Neither strategy should win by construction in every environment.

Start with a small, legible set of founder capability profiles. Let heritable
variation change capabilities within explicit costs, rather than making categories
permanently immutable or expecting one unconstrained continuous slider to generate
all niches. Appearance should remain interpretable as lineages evolve.

The RNN chooses where to move, what accessible food to attempt, whether to rest,
hunt, court or reproduce. The body and world enforce what is physically possible
and what it costs, equally for neural and legacy control. Neural outputs cannot
override digestion. No scripted “leave this patch now” rule is required.

## Interdependence through consequences

Plant production feeds herbivores directly and the litter community after shedding.
Detritivores fragment and process litter; an initially implicit microbial community
provides background decomposition and nutrient return. Animal processing can change
the rate and location of recycling. Plants then depend on replenished local
nutrients. Microbial biomass need not become thousands of rendered agents.

Nutrients can cycle; usable energy must be spent and dissipated at each transfer.
Reprocessing feces or carcasses must never recharge their original food value.
Microbial conditioning may make remaining resources more accessible while consuming
some of them. It is not an energy bonus for waiting.

Fruit consumers could carry seeds and deposit them after travelling. That would
make fruiting, movement and plant recruitment one interaction rather than three
unrelated animations. Introduce actual seed transport if this role is chosen; do
not award an abstract plant-growth bonus for having a frugivore nearby.

Predators depend on accessible prey and can alter grazing pressure and prey movement.
Prey size, handling costs, cover and escape opportunities create limits. The apex
must have a supportable place in that web; its presence does not guarantee balance.

Avoid a brittle dependency in which one named animal disappearing permanently
switches off all decomposition or plant reproduction. Background pathways and
partial functional overlap can permit recovery while specialists still make a
material difference. Local losses and succession are acceptable; permanent sameness
is not the objective.

## Space, population and time

Food should be patchy, finite locally, and recover over meaningful travel and life
times. Light, moisture, cover and nutrient availability create habitat differences.
No invisible face boundary or compulsory preferred destination is needed: local
sensing, memory and differing food access can produce habitat use.

Select resource renewal relative to consumption and maintenance, and reproduction
relative to food surplus. Parents pay for offspring material, energy and gestation;
juveniles still need food to mature. Reserve capacity and developmental times set
the lag between a surplus, births and later starvation. A finite initial litter
stock may produce a legitimate boom, but cannot be mistaken for ongoing carrying
capacity. Estimate supported biomass from sustained accessible production, not
starting stock or a desired head count. Do not impose an arbitrary fixed trophic
efficiency or predator quota.

Make each resource readable: grazed patches, litter, carcasses and plant recovery
need visible representation proportional enough to understand feeding. Diagnostic
resource maps and intake ledgers belong in explicit tools; the ambient display
should communicate through the organisms and habitat themselves.

## Evidence informing this proposal

- Anderson, Pond & Mayor's [2017 detritivore nutrition model](https://www.frontiersin.org/journals/microbiology/articles/10.3389/fmicb.2016.02113/full)
  distinguishes direct detritus consumption from microbial feeding pathways. It
  shows that microbial nutritional benefits interact with losses through respiration
  and food quantity. This supports representing food quality and finite energy;
  its aquatic micronutrient model is not a numerical calibration for Cubarium.
- Morris, Allhoff & Valdovinos' [2021 evolving-food-web model](https://www.nature.com/articles/s41598-021-99843-3)
  finds that disturbance can favor generalists even with an imposed efficiency
  trade-off. This supports testing specialization under varied resource conditions;
  it does not show that adding strict diets will automatically stabilize a world.
- García-Oliva & Wirtz's [2025 aquatic food-web study](https://www.nature.com/articles/s41559-025-02647-1)
  combines feeding constraints and specialization to describe observed trophic
  links. It supports trait-based access to prey over universal edibility. Its
  aquatic size relationships should not be copied directly into the cube.

The resource categories, founder roles, plant structure, seed transport and staged
workflow above are design proposals informed by these principles, not conclusions
that the papers prove necessary.

## Next reviewable milestone

Before another learning campaign, produce a compact food-web contract: the proposed
resources and consumers, capability trade-offs, paid reproduction, renewal and
recycling pathways, and the intended visible consequences. Choose the minimum set
of mechanisms that makes those differences real. Map implementation changes only
after the ecological model is reviewed.

Then validate one resource-consumer interaction at a time, with measured intake,
maintenance, depletion and renewal; add the smallest coupled community. Check that
removing a food source affects its dependent consumers and that removing a recycler
changes recycling measurably without granting or removing energy by decree. Use
matched paid controls to separate resource insufficiency from poor foraging.

Only then train controllers within the agreed bodies and food web, progressing to
competition, predation and reproduction. Existing RNN/ES work remains useful
infrastructure; saved policies may require retraining. M1 can later tune sustainable
regimes of this model, but should not be asked to discover missing ecological roles
by adjusting today's knobs.

This work used three primary research papers and existing repository evidence. No
experiments, implementation, delegation or live changes were performed. Billed
token usage is unavailable.
