---
design_status: exploration
last_reviewed: 2026-09-16
decision_refs: []
---

# Cubarium: grow a world, guide its evolution

Roadmap exploration following Wrysk's game pitch on 2026-09-16. The owner wants
an actual game and a potential dedicated simulation-hardware product. A barren
opening, starting budget, genome points earned through successful reproduction,
biological upgrades, a science tree and terraforming devices are candidate
mechanics. Exact currencies, unlocks, objectives and balancing are not decided.

**This does not change the immediate trajectory:** local desktop development of
the shallow voxel ringworld, terrain and water, as described in the
[landscape proposal](terrain-and-ecosystem-proposal-2026-09-16.md). No game system,
editor, training campaign or deployment is implemented or launched by this note.

## 1. The game promise and core loop

**Build a habitat, establish life, and use its success to expand what your world
can become.** The player starts with a mostly barren landscape and limited
means. They shape water and terrain, introduce organisms, watch what succeeds,
and guide the next generation. The resulting world can be played actively or
kept running as a living desk object.

Proposed loop:

1. **Read the landscape.** Find water, sunlight, usable soil and shelter.
2. **Invest.** Sculpt a small habitat, place a device, introduce a founding
   population or give a lineage a new biological option.
3. **Observe consequences.** Water moves, plants establish, creatures feed,
   compete and reproduce. Fast-forward makes delayed consequences accessible.
4. **Earn progress from establishment.** Successful reproduction and descendant
   survival reveal that something works; these events can award genome points
   and unlock research opportunities.
5. **Choose a new possibility.** Expand a habitat, open a new ecological role or
   change an inherited trait. The environment presents a different problem.

At short timescales, placing a channel should visibly redirect water. Over a
session, a new population should establish or make its failure understandable.
Across sessions, lineages and recognizable places should acquire history. These
are experience targets to test, not scripted biological outcomes.

The game needs to make ecological cause and effect understandable. A player
should be able to inspect why a plant cannot establish or which food a creature
can reach. An optional field guide and concise diagnosis can provide this;
the ordinary landscape remains free of permanent analytical overlays.

## 2. A first playable story

One candidate opening: a rocky ringworld with a low aquifer, exposed sunny ground,
some soil and a shallow depression. The player has enough initial budget to
establish one viable community. “Barren” does not mean no nutrients, no viable
water route and no way to start photosynthesis.

They form a small pool and wet margin, introduce a pioneer plant and a small
grazer population, and accelerate time. New growth supplies food; successful
offspring provide the first genome reward. A later unlock permits a pump and
solar panel. Pumping creates a spring on an otherwise dry terrace, making a
second habitat possible. They then choose whether to introduce a new producer,
guide an existing lineage toward that habitat, or improve its water storage.

This is a playable scenario to construct once the physics and small community
work, not a promise to build the whole science tree first. Weather, starting
resources and unlocked tools must make its intended solutions feasible.

## 3. Progression should reward the ecology the player wants

Wrysk's proposed genome points give reproduction an immediate game meaning.
The central design risk is that paying equally for every birth could make a
short-lived reproductive swarm more profitable than establishing a varied world.

Candidate reward designs to compare, without selecting one yet:

- A modest birth reward, with a larger reward when offspring mature or themselves
  reproduce. Keep early feedback immediate while valuing completed life cycles.
- Milestones for a lineage establishing in a new habitat, a new functional food
  relationship, or recovery after a disturbance. Attribute these to measured
  events rather than arbitrary colour/genome differences.
- Diminishing returns from repeating an already demonstrated achievement, if
  repetition overwhelms progression. Do not punish a healthy stable habitat
  merely because it is familiar.

Reproduction still costs real biological resources. Genome points are an abstract
player currency; they are not nutrient, food energy or a hidden breeding subsidy.
The game may explicitly grant seeds, animals or supplies, but those are named
world inputs. Keep three different concepts clear:

| Concept | Function |
| --- | --- |
| Genome/research progress | Unlocks or purchases new intervention possibilities |
| Starting/build budget | Limits initial terraforming, equipment or introductions; could share a currency with research if simpler |
| Simulated resources | Water, nutrients, biomass, food energy and any device power; constrain what actually happens |

There is no need to implement three player-facing currencies. First test whether
one visible progression currency plus physical constraints creates useful choices.
Do not let the accounting table grow into an elaborate economy by accident.

Rewards follow simulated ecological events, so faster play reaches them sooner
in wall time but does not multiply their value per event. Event identity and
reward settlement must survive save/load without paying twice. Developer tools
can grant resources or unlocks explicitly; production research gates must not
hide unfinished features from development testing.

## 4. A science tree of ecological possibilities

Prefer unlocks that enable a new relationship or landscape, with several viable
branches, rather than only increasing every species' numbers.

| Branch | Candidate unlocks | New possibility |
| --- | --- | --- |
| Water engineering | Channels, weirs, cisterns, pumps, condensers | Sustain a wetland, irrigate a terrace, buffer drought |
| Energy | Solar panels, batteries, more efficient drives | Power irrigation through varying daylight |
| Ground and shelter | Soil amendment, porous structures, retaining walls, artificial reefs | Create rooting space, refuges and different water retention |
| Plant biology | Root depth, drought tolerance, shade response, dispersal, flowering | Establish different producer communities |
| Animal biology | Digestive capabilities, locomotion, sensing, reserve strategy | Exploit new resources and connect habitats |
| Ecological relationships | Pollination, seed transport, fungal processing, aquatic life | Add useful pathways through the food and habitat networks |
| Later science fiction | Controlled grow lights, climate structures, advanced habitat engineering | Build environments the natural terrain cannot sustain alone |

These are illustrative nodes, not a content commitment or prerequisites to
write into the simulator now. Solar panels should follow simulated irradiance;
storage allows a pump to run at night. Device power can be a small bookkeeping
model without detailed electrical simulation. Equipment capacities and costs
must be visible enough to plan around.

Biological upgrades need a clear intervention model. Candidates include editing
a selected lineage's future offspring, introducing an engineered founder, and
applying breeding selection over generations. Start by prototyping one. Existing
adults should not silently acquire a different body unless that is an explicit
game action. A broader diet or drought tolerance can carry physiological costs;
research can expand the design space without abolishing ecological tradeoffs.

The player guides evolution through both genes and habitat. Creating a wet
corridor may change which inherited variants succeed without spending points on
every individual trait. The game should distinguish engineered changes from
changes that arose through inheritance and selection.

## 5. Active game and ambient world

The same world can support active building and quiet observation. These are
different ways to engage, not separate physics implementations.

| Control | Meaning |
| --- | --- |
| Pause / single-step | Development, inspection and optionally planning |
| Accelerated time | Execute more simulation steps per real second; rendering can skip intermediate states |
| Real-time mode | Run at the intended ambient rate; optionally synchronize the environmental day to local time |
| Follow simulated lighting | Visually show the world's current day/night conditions |
| Constant presentation lighting | Keep the landscape readable while ecological day/night still runs underneath |

Use fixed simulation steps when accelerating; increasing the physics timestep
would change water and ecology, making fast-forward a different set of rules.
Maximum speed depends on measured desktop/device throughput, not a promised
multiplier. Population growth and increasing active water volume may change it.

Separate **simulation time**, **environmental solar phase**, and **display
lighting**. In accelerated mode, a world day advances with simulated time. In
real-time synchronization, environmental phase follows the configured local
day; a timezone and configurable schedule suffice initially. Matching actual
sunrise/sunset by season/location is a later optional refinement.

When returning from fast-forward to a real-time phase, adopt an explicit
transition policy; do not rewind organisms, water or already-earned progress.
One candidate is to preserve current phase and smoothly reconcile the light
cycle with local time. Record the forcing used so headless evaluation can replay
it. Clock changes must not accidentally create a long catch-up or double rewards.

Turning off *visual* day/night only changes presentation. Photosynthesis, solar
generation and nocturnal behavior still use simulated light. Actual grow lights
are a separate world intervention with an explicit energy source. The UI needs
to explain this distinction once, without exposing implementation details.

Unattended running is part of the product, but shutdown/offline simulation policy
remains open: pausing and bounded catch-up are different experiences. Do not
infer a requirement for unbounded offline execution. Test ordinary absence and
recovery so the desk object does not demand a daily maintenance obligation.

## 6. AI that belongs in the game

The most relevant AI proposition is already connected to the simulation:
**creatures inherit learned behavior, and their success depends on the habitats
and communities the player creates.** It would need evidence of useful behavior
and generalization in the new ecology; existing controllers do not prove that
the proposed game already has adaptive intelligence.

Three different possibilities should retain accurate names:

1. **Neural creatures:** offline-trained or evolved policies used by living
   organisms, potentially inherited/mutated through reproduction. Inherited
   evolution is different from learning new weights during an individual's life.
2. **Ecosystem discovery:** offline search finds diverse viable combinations of
   traits, policies, terrain and climate. It is development/content tooling,
   not evidence that a live world is open-ended or self-balancing.
3. **Optional builder/naturalist assistant:** later, help a player draft terrain,
   interpret observed outcomes or propose a habitat intervention. Advice should
   use the actual world state, and generated worlds must pass ordinary structural
   validation. This is not needed in the simulation's per-tick loop.

The first two build directly on the existing recurrent-policy and ecosystem
research. The third is a later product experiment, not a reason to add a cloud
dependency now. Keep the core local/offline capable. Any pitch should say which
capabilities are implemented, trained, demonstrated or merely planned. The
product story should grow with what the creatures and tools can demonstrably do.

## 7. What this changes now—and what waits

The terrain/water milestone stays first. Its useful planning implications are:

- Generated and authored terrain use the same voxel/material representation.
- Player and developer interventions enter through explicit world commands.
- The simulator emits observable lifecycle/resource events; a separate game
  layer can award progress and authorize commands. Points do not enter food or
  fluid equations, and the simulator can run without campaign progression.
- Simulation speed, environmental phase and presentation lighting are distinct.
- Devices have real intake/outlet/capacity semantics, so a later science tree can
  unlock them without rewriting water accounting.

These are interface considerations, not a request to implement a general-purpose
event framework, reward economy, campaign, editor or assistant this cycle.

After terrain/water and a small community are usable, build one compact playable
scenario: **a barren terrace becomes a reproducing habitat, earning one meaningful
unlock that enables a second habitat.** Include a pump/solar option only once its
basic device rules exist. Test that loop before committing to a large science tree.

Observe whether players understand why their intervention worked, choose
different plausible next steps, use both acceleration and quiet observation,
and care about the resulting place. Also test whether birth farming, one
universal upgrade or idle waiting dominates the experience. These are product
questions; ecological numerical stability alone cannot answer them.

The pitch now has a concrete loop and a distinctive pairing of active creation
with an ambient hardware world. Enjoyment, retention, market demand and willingness
to buy dedicated hardware remain hypotheses to test. The current planning work
neither establishes them nor requires a market study before the physics prototype.
