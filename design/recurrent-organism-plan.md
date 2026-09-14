---
design_status: exploration
last_reviewed: 2026-09-14
---

# Recurrent organisms: bodies, senses, learning, and ecology

Implementation proposal for review, extending [movement and foraging](movement-and-foraging-plan.md).
Wrysk has requested recurrent control from the outset, without an MLP comparison.
That direction is explicit; the architecture, numbers and training protocol below
are engineering proposals. Nothing here schedules agents, starts training,
changes the live world, or adds accepted ledger entries.

**Recommendation:** one small GRU architecture, private memory per animal, a
local sensory interface, and bounded physical action requests. Establish honest
movement and finite feeding patches first, evolve useful recurrent behavior
offline, then introduce inherited variation and interacting diets. Whole-world
sustainability tuning with M1 comes afterward.

## 1. What the research supports—and what it does not

| Evidence | Consequence for this proposal | Limit |
| --- | --- | --- |
| [Ni, Eysenbach & Salakhutdinov, recurrent RL for POMDPs, 2022](https://arxiv.org/abs/2110.05038) found that carefully implemented recurrent policies perform strongly across several partially observed tasks; context length and implementation matter. | Use recurrence directly and test history-dependent behavior. | Neither GRU size nor ecological memory duration is established for Cubarium. |
| [Cho et al., 2014](https://arxiv.org/abs/1406.1078) introduced gated recurrent units; the [GRUCell equations](https://docs.pytorch.org/docs/2.14/generated/torch.nn.GRUCell.html) give an explicit implementation reference. | Use a fixed GRU cell, with a documented update convention. | Its origin in sequence modeling is not proof of ecological intelligence. |
| [Such et al., Deep Neuroevolution, 2017](https://arxiv.org/abs/1712.06567) demonstrates direct GA training of neural policies. [Salimans et al., 2017](https://arxiv.org/abs/1703.03864) demonstrates parallel evolution-strategy policy optimization. | Evolving weights in the real CPU simulation is a credible first route. | Neither paper predicts Cubarium's sample requirements or proves GA is the fastest optimizer here. |
| [Lehman et al., Safe Mutations, 2018](https://arxiv.org/abs/1712.06563) shows the sensitivity of deep/recurrent policies to mutation and benefits of output-sensitivity-aware operators. | Measure functional disruption and begin with conservative mutation. | Simple perturbation rescaling is not equivalent to their stronger gradient-based method, and neither guarantees ecological viability. |
| [Heess et al., 2017](https://arxiv.org/abs/1707.02286) demonstrates varied locomotion emerging in rich environments under a simple objective. | Put meaningful challenges in the environment rather than reward every prescribed gesture. | This is locomotion evidence, not a demonstrated sustainable food web. |
| [Mouret & Clune, MAP-Elites, 2015](https://arxiv.org/abs/1504.04909) maintains high-performing solutions across chosen behavioral dimensions. | Retain multiple viable behavioral candidates instead of just one champion. | Descriptor diversity does not guarantee ecological coexistence. |
| [Suarez et al., Neural MMO, 2019](https://arxiv.org/abs/1903.00784) studies persistent multiagent worlds with finite resources. | Evaluate interaction and competition after individual capability. | Its training infrastructure and respawning conventions are not a Cubarium design contract. |

The specific recommendations below are inferences from these results and this
repository's constraints. No paper establishes that recurrent brains alone will
make this world interesting, intelligent, or sustainable.

## 2. Separate physical possibility from behavioral choice

The world owns body motion, collision/contact, sensing range, resource transfer,
digestion, paid reproduction and death. The policy chooses motor and feeding
effort. It cannot set a position, instantly change heading, create reserves,
declare a capture successful, or bypass a gestation cost.

Pivoting is valid locomotion. Use the candidate bound
`|v| + r × |ω| ≤ u_available`: center velocity and outer-body rotational sweep
share available movement capability. Pure rotation is allowed at zero center
velocity. The body's current extent determines `r`; energy, water and morphology
constrain capability. Apply linear/angular acceleration limits and charge both
translation and rotation. A resting animal has neither motion. This is a
conservative stylized budget, not a rigid-body solver or car steering model.
Specify a small shared motor deadband/breakaway threshold so near-zero neural
outputs can produce actual stillness rather than permanent subpixel jitter.
This belongs to actuator semantics, applies to every controller, and must not
become a hidden behavioral rest timer.

Use the same motor resolver for ordinary movement, apex pursuit, escape and
encounters. Existing code that directly replaces heading or forces a perched
mode must not secretly control an RNN animal. World-enforced windup, digestion,
contact and funding constraints remain. Frame interpolation follows resolved
motion, not a separate visual steering process. Cube chart transport is not a
physical pivot and must not incur a spurious turn cost.

Food is finite within actual feeding reach. A productive visit consumes faster
than local renewal; depleted food recovers while the animal is elsewhere.
Establish this stock/flow relationship before neural training. Keep recovery
funded by nutrients, light and surviving producer tissue; test whether existing
stock/refuge rules suffice before introducing another resource pool. Coarse
cells must not grant access to untouched food beyond mouth reach. Plants should
show depletion and recovery on the underlying ecological schedule.

These rules should make stationary feeding unsustainable after a patch is
consumed. They should also permit a well-fed animal to rest. Do not impose a
universal wandering timer, perpetual motion reward or compulsory destination.

## 3. The first brain

### Completion scope

Reproduction is part of R3, including ordinary budding and apex mating, paid
gestation/birth, inherited policies, juvenile development and descendants that
can themselves reproduce. Births are disabled only in the isolated R2 foraging
fixture. R3 must evaluate reproductive lineages; its objective cannot remain
terminal reserve maximization, which could favor animals that never reproduce.

The intended neural behavior scope covers locomotion and exploration, habitat
responses, feeding and patch departure, resting, pursuit/attack, escape/retreat,
and reproductive readiness and mate approach/acceptance. R3 must explicitly
specify mate selection and refusal; an automatic nearest-partner rule must not
silently substitute for behavioral choice. The initial eight-output schema is
a starting interface and may need a versioned extension for those interactions.

Mechanical and physiological rules remain implemented by the world: reach,
contact, capacity, digestion, maintenance, gestation, growth, damage and death.
Existing dormant-offspring emergence also remains a rule-driven physiological
exception in this proposal. R3 must test that this lifecycle closes and expose
that exception clearly; neural control of dormancy/emergence is not currently
specified. It would require an explicit sensing/update/energy contract while
dormant, not merely another output on an inactive controller.

R3 finishes with a controller-ownership audit of every remaining legacy heading,
mode, feeding, attack, mating and retreat override. Each must be replaced by a
neural decision or identified as a deliberate physical/physiological constraint.
Do not claim complete neural control while unlisted heuristic decisions remain.
New capabilities such as parental care, communication or nest building are not
promised by this plan; their senses, actions and consequences do not yet form
part of this behavioral contract.

| Choice | Proposed starting contract |
| --- | --- |
| Architecture | One GRU layer, 32 hidden units, linear action head; no MLP comparison, topology evolution or extra perception network |
| Inputs | Approximately 64–96 bounded local and internal values; freeze the exact versioned layout in milestone R0 |
| Outputs | Eight physical effort channels described below |
| Update rate | 10 Hz initially, every second 20 Hz simulation tick; motor resolution and physical costs still run every tick |
| Arithmetic | Native Rust, `f64`, explicit deterministic loop order; no training runtime in the display process |
| Live weights | Fixed during an individual's life in the first version |
| Memory | 32 hidden values per animal, plus last action/feedback and controller cadence state |
| Variability | Different inherited weights and body traits, separate experience and optional explicitly seeded policy noise |

The GRU's gates let its state retain or replace information. They do not promise
memory over a particular duration. At 10 Hz, ten seconds spans 100 recurrent
updates and a minute spans 600. Start initialization with a range of retention
biases and small nonsaturated weights; verify rather than assume useful memory.
The rationale for attending to timescales is supported by
[Tallec & Ollivier, 2018](https://arxiv.org/abs/1804.11188); the initialization
range is a Cubarium choice, not a result imported from that paper.

Use the reset-after convention documented by GRUCell: the reset gate multiplies
the recurrent candidate contribution, and the update gate weights the previous
hidden state. Fix gate order, biases, activation functions and tensor layout in
the model format. Check against a small independent numerical reference. A
future training backend must export the same equations, not just something
called a GRU.

With two bias vectors per gate, 64 inputs and eight outputs give
`3 × 32 × (64 + 32 + 2) + 8 × (32 + 1) = 9,672` parameters; 96 inputs give
12,744. The smaller option uses about 76 KiB of `f64` weights and 256 bytes of
hidden state. Even 512 distinct smaller policies use about 38 MiB of raw weights;
this is arithmetic, not a runtime benchmark. Measure sensor construction,
network inference and complete-world throughput separately. Shared weights may
be deduplicated, but hidden state must never be shared.

## 4. Senses and actions

All spatial observations are body-relative, local, and continuous across seams.
Use fixed angular sectors with smoothly weighted contributions, so minor turns
do not repeatedly swap a sorted target list. Preserve concentration magnitude;
normalizing every food gradient to a unit vector erases how weak the signal is.

| Input family | Contents and restrictions |
| --- | --- |
| Food | Local plant, fruit and usable detritus quantities; directional samples within paid sensing range; concentration at feeding reach |
| Other bodies | Sector occupancy/proximity and observable relative size/motion; observable compatibility cues where mating needs them, not hidden genome or energy values |
| Surroundings | Local water, light/habitat cues, up direction and contact/blocked-motion feedback |
| Internal state | Reserve, energy, gut fullness, development/reproductive condition and relevant body capabilities |
| Recent outcome | Actual intake and resolved motion since the last controller update, including rotation and failed movement |

The exact fields and normalization constants are part of the observation schema.
Bound by fixed physical scales; do not normalize using the population's current
statistics. Distinguish absent objects from low-valued features. Deduplicate
neighbors and specify occlusion/visibility assumptions. Do not expose global
food, future conditions, optimizer reward, an organism's slot/ID, or a hidden
"this is the correct prey" flag. IDs may remain internal deterministic handles.

The first food sensor can reuse local field sampling as an abstraction of
chemical sensing. It is not a physical odor plume. Actual diffusion, transport
or delayed smell would be a separate environmental mechanism, if later useful.

Eight proposed outputs:

1. Fore/aft translational effort, signed.
2. Lateral translational effort, signed, within the body's capability.
3. Rotational effort, signed.
4. Grazing effort.
5. Fruit-feeding effort.
6. Scavenging effort.
7. Attack attempt/effort where the body supports it.
8. Reproductive readiness/attempt where the lifecycle supports it.

Bound outputs and resolve simultaneous requests against real capacity. Separate
feeding channels must share mouth/handling capacity; movement and feeding cannot
independently spend the same energy. Unsupported channels are masked by body
capability, not by a hand-authored search/feed/rest controller. Digestion and
maintenance remain physiological processes rather than voluntary opportunities
to switch off upkeep. A continuous attack/reproduction request is interpreted
with explicit cooldown, contact and funding rules; it cannot retrigger every tick.

No discrete navigation target is required. Gradient following, persistent travel,
local search and target tracking can arise from sensory history. Avoid adding
secret attraction vectors or escape autopilots to make early policies look good.

## 5. Memory, inheritance, and persistence

Separate the inherited controller specification/weights from acquired hidden
state. Birth inherits weights with recorded mutation; hidden state starts at
zero. A snapshot restores the actual hidden state, last action, feedback
accumulators and update phase. Do not reset memory at a seam, meal, render frame,
or ordinary save/load. Death removes the owner's memory; reused arena slots get
new state. Dormancy freezes control updates and action execution initially;
resumption uses persisted state, with that convention tested explicitly.

Version and hash the observation layout, motor contract, GRU convention, rate,
weights and mutable state. Store exact values, not decimal approximations or a
path to an optional external weights file. Checkpoints must be self-contained
for surviving policies. Existing saves remain explicitly legacy-controlled
unless migrated under a chosen policy; loading a save must not silently replace
its brains or reset the world. Test same-build uninterrupted versus resumed
execution. Do not promise cross-platform bitwise equivalence from `f64` alone.

Initially, offspring copy one parent's complete policy plus bounded mutation.
For two-parent apex births, choose the policy parent deterministically from
recorded birth RNG; preserve both biological parents in ancestry. Do not start
with arbitrary elementwise weight crossover: recurrent units co-adapt and
whole-policy inheritance gives a simpler first contract. Body inheritance stays
under its existing explicit rules. This policy-inheritance choice is proposed,
not implied by accepting recurrent control.

Offline training, inherited evolution, and lifetime weight learning are distinct.
The first version uses offline training for viable founders, private recurrent
memory during life, and later weight mutation at paid births. There is no
background optimizer replacing weak live organisms, archive recruitment, or
fitness-based resurrection. Fitness exists only in the development trainer;
natural live selection occurs through actual survival and reproduction.

## 6. Training route and objective

### First optimizer: mutation and selection in the real core

Use a small population GA with elitism, deterministic seeds, and mutation-only
reproduction. Parallelize independent episodes on CPU. Run the same sensor,
network and motor code that will run on the display. M1 can later reuse the
evaluation boundary; do not force neural weights into its current ecological
parameter vector or modify its scoring during this phase.

Start with fan-in-scaled initialization and conservative blockwise Gaussian
mutation. Measure action divergence on a small bounded set of recorded training
sequences, rolling each candidate's own hidden state from the same initial state.
Up to two deterministic rescalings may reduce a disruptive perturbation; record
the effective mutation and unchanged proposals. This is a cheap SM-R-inspired
precaution, not a claim to reproduce SM-G or to guarantee future behavior.
Do not use held-out traces to select or rescale mutations. Include this work in
the wall-clock and policy-step counters.

If mutation destroys useful memory or progress stalls, stop with the evidence.
The next bounded choice is sensitivity-scaled mutation using output gradients,
or recurrent policy-gradient training—not an automatic seed/architecture sweep.
[PPO](https://arxiv.org/abs/1707.06347) is one possible policy-gradient route;
the recurrent-RL evidence above also supports considering an off-policy method.
No winner between them is established for Cubarium. Gradient-based policy
training does not require a differentiable environment, but does require correct
sequence handling, hidden-state reconstruction, training masks and terminal
boundaries. If introduced, use a separate critic rather than adding critic
weights to the live organism.

### Train for viable resource acquisition, not movement for its own sake

Begin with one mature body/diet and finite real fields, with body traits and
ecological parameters identical across candidate evaluations. This is an
explicit individual-capability fixture, not a sustainable ecosystem claim.
Disable births in this fixture equally for all candidates, so reproduction
cannot confuse the first foraging signal. Keep all physical maintenance and
movement costs. Initial resources and any fixture edits must be accounted.

Use fixed paired layout families with different headings, patch positions and
local availability. A nearby opportunity provides a discoverable starting
signal, while depleted patches and separated food demand continued foraging.
Include absence of food, strong versus weak local cues, and situations where
remaining at the initial patch cannot sustain the episode. Prove the fixture
requires feeding by running a no-intake control: starting reserves alone must
not cover the evaluation horizon. If it does, repair the fixture before training.
Permanent no-food/no-intake cases are negative controls with expected starvation,
not solvable training cases that a successful organism is required to survive.

First selection uses survival across the layouts, then bounded terminal usable
stores among equally surviving candidates. Record per-layout outcomes and
worst-case results, not just a favorable average. Death ends the focal episode;
no respawning or post-death rewards. Intake, expenditure and store changes are
audited separately so taking energy and wasting it is not scored as success.
The precise ordering and normalization are frozen with the first protocol.

Path length, spin count, feeding mode, raw births and frame animation receive no
direct reward. Departure and fresh-ground coverage are behavioral evidence.
An organism may stay at a genuinely productive patch until leaving is useful;
the environment must establish when that time arrives. Do not secretly increase
food or lower costs for a struggling candidate.

If sparse survival feedback fails, report that before adding shaping. Any new
shaping is a separate recorded objective change and must be evaluated without
its bonus. Simple energy/reserve bonuses can select hoarding or short-horizon
strategies, so success in this fixture does not substitute for reproduction and
multi-generation testing later.

## 7. Preserve behavioral variety

Initially retain a bounded set of viable candidates with different measured
behavior, not just the scalar winner. A small MAP-Elites-style grid can follow
once viable policies exist: proposed axes are spatial coverage during active
foraging and residence time at genuinely productive patches. Apply a viability
floor first; spinning or starving cannot earn a niche merely by being different.
Use deterministic descriptor windows and cap the set at 16 policies initially.
These descriptors express a development preference, not natural ecological
species or a proof of coexistence.

The set is an optimizer data structure containing compact weight files and
metrics, not a growing collection of binaries or captures. Keep chosen founders
and reproducible source/config in Git and clean task-owned scratch data. It does
not automatically reseed the live world.

Extend next across diets, then predators and prey with alternating bounded
training against multiple fixed opponents/foragers. Keep a few prior opponents
within the same cap so improvements are tested beyond a single adversary.
Evaluate the resulting mixed world with all policies fixed before enabling
mutations at paid births. Do not optimize the whole food web against one frozen
prey policy and call it coevolution. Lifetime weight learning and morphology/
controller coevolution remain later milestones.

## 8. Evidence required before declaring success

| Check | What it establishes |
| --- | --- |
| Shared movement budget, pure pivot, zero-energy motion, contact and seam fixtures | Every controller is bound by the same body rules |
| Reach-limited consumption, competing consumers, recovery and accounting checks | Local food is finite and costly behavior is funded |
| Independent GRU reference, fixed input sequence, mutation replay, save/load continuation | The recurrent implementation and persisted history are correct |
| Two histories ending in the same current observation | The controller interface supports different actions based on memory |
| A trained history-dependent food task, followed by hidden-state reset as a diagnostic | Whether the learned behavior actually uses memory; resetting is a diagnostic, not an MLP comparison or live behavior |
| Untouched layouts/headings, a seam layout, a depleted opening and longer return opportunity | Transfer beyond the training geometry and initial food |
| A bounded sample of raw decisions versus resolved movement/intake | There is no hidden heuristic providing the apparent intelligence |
| Native-size observation of ordinary mixed fauna | Useful travel, feeding, pivots and rest read on the actual display |

The history-dependent task must remove current cues that reveal the answer, for
example an earlier local cue indicating the productive branch followed by an
ambiguous junction. Keep this diagnostic distinct from the natural food field;
it proves memory use, not biological smell fidelity. Failing this diagnostic is
a reason to investigate training/history, not to add an MLP arm.

Measure displacement in body lengths, distinct ground reached, turning sweep,
time near the starting point, actual intake, energy expenditure and patch stocks.
Long circular paths cannot masquerade as exploration. These metrics belong in
development tools/logs; normal display remains free of analytical UI.

## 9. Bounded delivery and handoff boundaries

Each row is a separate handoff after review of the preceding result. One worker
owns shared world-step integration at a time. A pure GRU component can later be
delegated independently only after its interface is frozen. Use fresh bounded
GPT/Opus contexts or external handoffs per Wrysk's chosen routing; do not spawn
the whole plan in parallel. Each milestone gets at most one targeted review and
two repair cycles, then a spending checkpoint.

While awaiting Fable's broader review, the proposed early start is the narrowed
[R0a Opus handoff](handoffs/r0a-movement-foundation-2026-09-14.md): implement shared
physical movement limits and measure local food stock/flow. It does not commit
to R0's complete resource redesign or freeze the neural senses/actions schema.

| Milestone | Concrete deliverable | Stop condition |
| --- | --- | --- |
| R0 — Body and resource contract | Pivot-capable motor resolver; finite reachable feeding and recovery; observation/action schema; simple scripted probes of physical capability | Movement, contact, resource and native-size demonstrations pass; no learned behavior claim |
| R1 — Recurrent runtime | GRU32, local sensory encoding, private persisted state, birth/death handling, explicit legacy migration, model format and replay tests | Same-build continuation and reference checks pass; runtime/memory measured |
| R2 — Bounded foraging training | Deterministic trainer using the real core, one training screen and untouched evaluation; compact policies and component outcomes | Demonstrated foraging/memory use or a specific failure report; no automatic expansion |
| R3 — Diets, apex and inheritance | Multiple viable policies, honest hunting/escape, paid reproduction with weight inheritance, mixed-world review | Individual activity transfers to ordinary interactions; diversity and offspring outcomes reported |
| R4 — Sustainability search | Extend M1 to evaluate the new biology and tune ecological parameters with held-out longer runs | Report sustainability evidence and failures; no indefinite-survival promise |

R0 and R1 can use the current controller only as an integration reference, with
minimal adaptation and explicitly scripted physical probes. Do not implement a
second neural architecture or spend a milestone perfecting old behavioral rules.

### Proposed compute limits, to finalize from R1 measurements

- R1: one short performance screen at 32 and 128 bodies for 2,000 ticks each,
  plus a 512-body 200-tick stress check. These are performance fixtures, not
  ecology. Cap the combined screen at 60 seconds wall time and record incomplete
  checks. Measure inference and total ticks/second; do not extrapolate the old
  non-neural M1 throughput as a new benchmark.
- R2 plumbing smoke: at most eight episodes, 2,000 ticks each, four CPU workers,
  60 seconds wall time. This verifies plumbing, never learning.
- First R2 learning screen, only after the compute checkpoint: 32 policy slots,
  at most 16 evaluated generations including initialization, four fixed training
  layouts each, 12,000 ticks/episode, at most eight workers, and 20 minutes total
  wall time. That is at most 2,048 episodes / 24,576,000 world ticks. All retries,
  failures and mutation rescaling work count; poll cancellation within episodes.
- Held-out screen: at most four selected policies on eight untouched layouts,
  12,000 ticks each, 32 episodes / 384,000 ticks and two minutes wall time. Freeze
  selection before opening these results; do not repeatedly retune on this set.
- The 600 simulated seconds per episode is provisional. R0/R1 must establish
  that it includes depletion and travel and outlasts no-intake survival. If it
  does not, revise the protocol at the checkpoint, not after seeing winners.
- Stop if limits are reached. These caps may be too small to learn; a failed
  screen is evidence about this configuration, not proof neural control fails.
  Do not automatically add GPUs, an optimizer sweep, more seeds or a new model.
- One normal build cache, compact results targeted below 10 MiB per screen, and
  a checkpoint before adding over 1 GiB of storage. No capture archive or frozen
  executables. Delete only task-owned temporary files within the agreed scope.

The runtime code should support later GPU training, but no GPU simulator port or
new ML installation is required for the initial CPU neuroevolution proposal.
Adopt gradient training only with its own dependency, throughput and equivalence
checks. The old M1 GPU assessment does not determine whether GPU neural training
would be useful; those are different workloads.

## 10. First handoff content and repository touchpoints

After plan review, hand off R0 only: this document, the movement overview, root
working rules, and the current relevant source spans. Assign one owner for motor
and feeding integration; require the observation/action schema as an output for
R1. Include the explicit exclusions: no MLP, no new heuristic behavior campaign,
no M1 changes, no training, no unsolicited reset, and no unrelated cleanup.

Current touchpoints to verify through Graft before editing are
`controller.rs::Observation/Decision` (local inputs and intents), the movement,
hunter override and feeding stages in `world.rs`, `fields.rs::react`,
`genome.rs`/`organism.rs` for inheritance versus lifetime state, snapshot
migrations/hashes, and presenters that consume movement/feeding state. These are
scope pointers, not permission to rewrite all of those modules in R0.

Relevant regressions include arbitrary heading overrides in hunter pursuit,
double-spent feeding capacity, angular travel misread as seam rotation, hidden
state leaking through reused organism slots, different GRU conventions between
trainer and runtime, and merely labeling an animal Feeding when actual intake
is zero. Update Graft after substantial implementation changes.

No implementation or numerical training results accompany this plan. The only
numerical claims here are derived parameter/storage counts and proposed caps.
