---
design_status: exploration
last_reviewed: 2026-09-18
decision_refs: []
---

# Voxel senses: primitives, bodies, and training arenas

**Orchestrator entry point — planning complete for phase one.** Wrysk requested
this finished design/planning handoff as the starting point for coordinating
implementation. Read the linked plans, then assign P1-A; routine choices do not
need another organism-by-organism approval round.

1. [Phase-one implementation plan](voxel-senses-phase1-plan.md): scope, current
   code pointers, static arena, exact first manifests, local actions, starting
   field coefficients, dependencies and work packages.
2. [Phase-one testing and expected outcomes](voxel-senses-phase1-tests.md): short
   correctness checks, arena tasks, heuristic controls, bounded ES pilots,
   deliverables and the distinction between working tooling and learned behavior.

The first slice is a blind litter feeder and a sighted browser in static arenas.
Their exact phase-one manifests and actions are defined in the implementation
plan; the broader catalogue below includes later capabilities. Nothing has been
implemented or trained by this planning work. No design ledger is changed.

Working proposal incorporating Wrysk's review of the first organism-by-organism
research draft. Foundations agreed in conversation remain: senses belong to
anatomy, blindness is viable, observations are egocentric, interpreted perception
is allowed, and each organism's controller owns behavior. No pathfinding,
external food-seeking controller, universal predator input, or global truth.

The revised primitive layouts and organ assignments below are proposals, not
accepted canon. Earlier agreement on the littershredder used left/right/trend;
the review proposes temporal chemistry as the smaller baseline and bilateral
sampling as a separate founder configuration. No ledger entry is changed.
No runtime, training, or display changes have been made for this document.

Scope is the seven animal roles in the
[theoretical biosphere](theoretical-biosphere-2026-09-16.md#seven-animal-roles),
under the [senses handoff](handoffs/voxel-senses-handoff-2026-09-18.md).
Electrical/magnetic senses and the ten sessile roles remain outside this pass.
Organ appearance follows the [art direction](art-direction/Cubarium_Art_Direction_v0.1.md).
Earth studies inform available information, not prescribed alien silhouettes.

## 1. Fixed catalogue and encoding

One primitive is a sensory module implemented by specified body tissue. Several
modules may share a physical appendage; account for its tissue once. A founder's
ordered module list, tunings, channel meanings, sector layout, action layout,
and cadence define its schema and digest. That list stays fixed within the
lineage. Continuous range, placement, sensitivity, and integration parameters may
mutate. Adding/removing an organ, chemical tuning, class channel, or sector makes
a new founder configuration and digest; it is not a structural mutation of a
running lineage. This can support a later sensing unlock without reinterpreting
old weights. It does not decide the game's unlock progression.

Output dimensions below include one validity value per module, except `Self`,
which is always available. Zero signal remains a valid measurement; invalid
modules return zero data and validity zero. Missing organs have no slots. If
parts can fail independently, represent them as separate modules rather than
pretending one validity bit can express independent failures.

| Primitive | Fixed scalar shape | Allowed information |
| --- | --- | --- |
| `Self` | 8 | Energy, reserve, reproductive readiness, own structural loss, settled intake, resolved forward movement, resolved turn, motor delivery fraction |
| `Chem(cue)` | 3: response, trend, valid | One local receptor response and its recent change |
| `ChemPair(cue)` | 5: mean response, trend, left, right, valid | Two physically separate samples; replaces `Chem` for that tuning |
| `Contact(n)` | n contact intensities + valid | Pressure/contact at n fixed body or appendage locations |
| `Taste(K)` | K cue responses + resistance + valid | Current mouth/probe-contact chemistry and mechanical resistance; invalid without contact |
| `Wet` | local wetness/immersion response + valid | Water at the actual skin, foot, or probe; not ambient humidity |
| `Tilt` | pitch, roll, valid | Body/support orientation from balance or load-bearing contact geometry |
| `Light` | intensity, valid | Local incident light at a light-sensitive patch; no shape or direction |
| `Tremor` | intensity, trend, valid | Short-lived substrate disturbance at a contacting receptor; no source identity |
| `Cone(S,K)` | S × (2 + 2K) + 1 | Per fixed sector: clear-ray fraction, mean hit proximity; K visible class fractions and their class-specific mean proximities; one organ validity |
| `BodyView(S)` | S × 5 + 1 | Per sector: detected coverage, azimuth, elevation, apparent angular extent, proximity of one current visible body patch; one organ validity |
| `Pose(n)` | n body feedback values + valid | Fixed named actuator/body states, e.g. grip load/slip or airborne state and vertical movement |

`Self` is an intended physiological interface, not a claim that the voxel animal
already models all eight states. Every value needs an actual body-state source
before training. Fixed readiness/structural-loss inputs are not inventions of mate or
predator locations. `Pose` field meanings are schema-defined, never a bag of
arbitrary internals.

**Encoding rule.** Responses are fractions of receptor saturation in [0,1];
signed signals are in [-1,1]. Scales are fixed in the primitive/schema, never
renormalized by an individual's current detection radius, phenotype maximum,
population, or local strongest source. A sensitivity mutation changes the
receptor transfer function before encoding; a placement/range mutation changes
what is sampled. Those mutations still change perception and can harm an
inherited policy. Stable units do not eliminate body/policy co-adaptation risk.
Start with low sensory mutation rates and compare against no-mutation controls.
Distance uses a fixed physical reference distance, not each animal's range.

**Sub-voxel sampling.** Read surface fields with connectivity-aware bilinear
interpolation on the current support layer, never nearest-cell sampling or
interpolation through a wall/other floor. A later volumetric field would require
trilinear interpolation. Single-receptor `Chem` works through turning and travel.
Choose `ChemPair` only when the placement and field gradient produce measurably
different readings. Half a voxel is a useful experiment, not a biological cutoff.
Small bodies can receive different interpolated readings without a wider body.

**Trend.** Let q be encoded receptor response. Smooth q over a starting time
constant tau = one reference adult body length / reference cruise speed; for a
pair use the mean response. Emit the signed rate of change of that smoothed
signal, multiplied by a fixed schema time scale and clamped. Initialize history
from the first sample. Tau may vary within bounded body parameters; encoding
never divides by the mutable tau. One-body-length smoothing is a proposed scale
to test, not a guarantee of an informative gradient.

## 2. Cadence, runtime, and costs

Propose a founder controller period of clamp(0.25 × L / v_cruise, 0.1 s, 1 s),
rounded to a whole number of simulation ticks. Keep that period fixed for its
schema; do not change it when an animal is hungry, stationary, growing, or born
with a slightly different speed. At 1 BL/s this is 0.25 s for every body size.
A snail gets 1–2 Hz only if its speed in BL/s is sufficiently lower. Evaluate
reaction/strike/landing failure before adopting the same cadence for fast roles.
Changing the period changes the GRU's physical memory horizon and is a schema
change requiring evaluation, not an automatic improvement.

Sample modules at controller ticks; aggregate actual intake/contact impulses and
movement over the intervening interval. Physics and damage continue every tick.
Cue fields may update more slowly: use their held state for both sensor and arena
rather than pretending it is continuous. No baseline per-feature sample-age
channel or visual track history. Phase-offset controllers can distribute load.

The current GRU32 has compile-time input/output dimensions and a 70-value input;
see [gru.rs](../crates/cubarium-core/src/neural/gru.rs) and the
[flat contract](recurrent-interface-contract.md). Its tensor shapes, sampler,
trainer and digest handling must be adapted before these variable-width schemas
can run. For hidden width 32, I inputs, and O actions, parameters are
96 × (I + 34) + 33 × O. Fewer inputs reduce work but do not prove learnability.
Identical schemas can share policy initialization or weights across roles;
incompatible shapes/semantics cannot directly share weights. Budget experiments
by archetype instead of presuming seven independent training campaigns.

Organ tissue has build mass and maintenance. Propose no per-read energy bill.
Active sensing is movement of a real appendage, with motor/time costs; it may
change sample placement or available contact. Sensor computing cost is measured
separately from biological upkeep. Presentation may reflect sensed activity,
but a decorative feeler sweep must not claim new sampled positions or free active
sensing. Functional motion and rendered anatomy must agree.

## 3. Material vision without a hidden visual controller

Replace undefined salience with a fixed ray fan against terrain, water surfaces,
flora geometry, surface resources, and animal occupancy. The terrain material
enum alone does not contain all these things: a shared sensory occupancy query
is required, including dynamic organisms. First-hit visibility provides occlusion;
illumination/contrast constrain recognition. No renderer readback is needed.

Available classes are rock/soil, water, wood, foliage, fruit, flower, body, and
litter. Each eye exposes a fixed subset K; undisclosed classes still obstruct.
A clear ray means no encountered surface within range, not guaranteed sky or
traversable space. Fractions use a fixed ray count. Missing class hits have zero
fraction and zero proximity. A no-hit distance is never treated as a nearby hit.
Class-specific distances avoid interpreting the distance to a rock as the distance
to a body. Coarse proximity is a granted depth-perception abstraction, with fixed
quantization/accuracy, not exact globally accessible geometry.

`BodyView` reuses a bounded ray fan and groups current visible body hits within
a sector, selecting the strongest current patch without persisting identity.
All five values describe that same patch. Apparent extent is the visible portion,
not the body's hidden full size. A different patch can win next tick; no velocity
is synthesized across that switch. It is an optional richer snapshot for strike
alignment, not binocular reconstruction or a target-selection behavior.

Start without expansion/range-rate/private tracks. The RNN receives stable
sector slots and snapshots. Test expansion first as a later bellwing/hunter
ablation if useful control fails; sector image expansion need not imply tracked
object identity, but object-specific rates require a separately specified
association/validity mechanism. The prior research supports useful visual motion
processing, not its necessity in the baseline. Hidden-state dependence in R2d
likewise does not establish that a GRU will learn landing or compensate for every
missing motion cue.

## 4. Concrete cue and vibration fields

Propose sparse **support-layer fields**, not a dense 3D air simulation. One field
value belongs to a support face and local near-surface medium. A flat floor is a
2D grid; stacked floors require separate nodes/layers. The current world explicitly
supports multiple exposed faces in one column
([world.rs](../crates/cubarium-voxel/src/world.rs)). Collapsing them to one value
would let a roof and cave floor share odor. This is a 2.5D surface approximation,
not a claim to simulate airborne plumes.

Cache a bounded-degree local transport graph when terrain/medium connectivity
changes. Edges express physical near-surface connectivity, independent of an
animal's walking/climbing ability. No destinations or routes are computed.
Litter-associated, fungal-associated, and carrion cues use these surface layers;
aquatic-film cue uses a separate graph of continuously wetted support surfaces.
A wet patch does not connect to another pond through a dry column. Fruit/floral
surface cues can be later channels near their actual emitting geometry; airborne
bellwings cannot read a ground field as if it were local air chemistry. Initial
bellwing therefore uses sight/contact, and fruit height is resolved by sight.
Conspecific cues are absent under the budding assumption below.

Each field update deposits source-dependent emission, applies decay, then a
bounded local spread step. Active nodes plus immediate neighbors are processed;
subthreshold residue is discarded. Weights must keep the scalar nonnegative and
spread stable. Choose a finite lifetime/hop support to bound footprint. Start
with 0.5–1 s field updates (10–20 ticks only at dt = 0.05 s), then check whether
source changes alias at the controller cadence. Stale cues after consumption are
allowed as decay, but depleted sources stop emitting. Emission, sensitivity,
attenuation and threshold jointly set effective detection distance. No per-animal
search over all emitters. Cue scalars are trace proxies, not edible material.

`Tremor` is a separate optional field: deposit a bounded pulse from actual
load-bearing movement/impact, scaled by body load and resolved motor activity.
A standing animal can receive it; airborne motion does not shake the ground
unless it contacts it. The source's own disturbance is included, so pausing can
improve observation. Spread/attenuate over a small cached connected-solid stencil
and expire rapidly. No water-only or air-gap transmission in this first model;
a submerged continuous solid can still transmit. It carries no species, source
count, predator label, or direction. Freezing is a possible learned response,
not a rule triggered by intensity. Update/accumulate often enough that a brief
pulse is not lost between controller ticks.

Cost: chemistry O(emitting sources + active nodes × channels × bounded degree)
per field update, then O(receptors × interpolation samples) per observation.
Tremor costs O(moving sources × bounded stencil + active decay + receptors).
Ray cost is O(rays × capped traversal steps), plus bounded local occupancy work.
None of these fields is free, and vibration is not proven the cheapest remote
sense until profiled. No per-animal scan proportional to world size.

`Light` may reuse cached local sky exposure for an explicitly sky-lit first model,
with available canopy attenuation. Cache hits are cheap; misses still cost work.
Sky visibility is not local irradiance and cannot detect a glowcap. Emissive light
needs an actual local light contribution. `Tilt` similarly needs actual body
orientation or supports under contacting limbs/skin: neighboring terrain beyond
contact is not free proprioception, and voxel stairs do not have an automatic
smooth slope. Current point-site bodies may lack a meaningful tilt measurement.

## 5. Bodies as lists of primitives

All lists are proposals. This pass offers budding/asexual reproduction for the
first sensory founders, matching current single-parent voxel births
([step.rs](../crates/cubarium-voxel-fauna/src/step.rs)). Mate search is consequently
not required. Sexual reproduction later needs a separately designed recognition,
encounter and mating mechanism; a conspecific cue would be neither universal nor
free. Isolated foraging arenas initially disable births.

Define proposed `Ground` as Self + Contact(4: front, left, right, underside) +
Wet + Tilt + Taste(1 relevant mouth cue): 8 + 5 + 2 + 3 + 3 = **21 values**.
This assumes those contacts/orientation are modeled; do not fabricate them to
fill a vector. Two taste cue channels add one value. The list describes sensory
function, not four externally controlled feelers or a mandatory visual design.
Phase one's upright horizontal-support bodies omit Tilt explicitly, giving the
blind founder **23** inputs including Light and the browser **37** inputs with
Cone(3, foliage/body). The 26/29 figures below describe later Tilt-equipped
reference manifests, not the first delivered networks.

| Animal | Proposed list beyond its body feedback/contact modules | First action/learning task |
| --- | --- | --- |
| Littershredder | Ground + Chem(litter) + Light: **26**; a Tremor-equipped lineage has **29** | Crawl/turn/shred inside a litter-cue patch, then find another |
| Capgnawer | Ground with Taste(fungus, litter) + Chem(fungus) + Chem(litter); optional Light/Tremor; later contact probe | Find fungus among wood; fall back to conditioned litter |
| Ripple snail | Ground, Wet tuned to contact immersion, Chem(film); optional solid-contact Tremor/Light | Scrape attached film; handle a wet/dry boundary |
| Frondgrazer | Ground + Cone(3, foliage/body); optional Chem(leaf-associated surface cue) | Reach low foliage, crop, and turn around contact obstacles |
| Seedporter | Ground with fruit/seed taste + Cone(5, wood/fruit/body) + Pose(grip load, slip); surface fruit chemistry optional when locally available | Climb toward visible fruit, contact it, handle/eat |
| Bellwing | Self + flight Pose + landing Contact + Taste(nectar,pollen) + Cone(5, flower/surface classes) | Approach, attach, feed, depart; expansion is a later ablation |
| Lanternjaw | Ground with animal-tissue taste + Cone(3, body) + Chem(carrion); Tremor candidate; BodyView(3) if coarse alignment fails | Stop/turn/strike a visible moving body; learn handling from contact |

K class identities, exact flight Pose fields, actions and sector angles must be
listed in each eventual founder manifest. Resolve these routine details from
the review, the body's implemented actions, and bounded learning checks; they
do not require individual approval from Wrysk. Three/five sectors and these sample counts are trial settings, not
research-derived optima. Sampling density/misses need measurement. A visual
material cone reports visible evidence, not food reachability, successful attack
probability, routes, or remaining stock. Contact/settled intake closes that loop.
The same primitive implementation does not make all animals' packets identical.

Wrysk's conversational choice (2026-09-18): substrate vibration sensing can be a
lineage difference. Keep Tremor out of the original littershredder's default
organs; a separately equipped lineage can detect unclassified disturbance.
Wrysk also agreed to a simple light-sensitive patch for the original
littershredder: local brightness only, with no shape or direction. It does not
report safety or humidity. No sense appears merely because its query is available.
Optional modules mean separate schema experiments, not organs toggled mid-lineage.

## 6. Arenas before ES

Build the arena harness before any training campaign. Start with a proposed
32 × 16 × 12 voxel world (width × height × depth), frozen terrain, precomputed
water, and frozen flora except the resource being consumed. Use the same sensor
queries, contact/feeding settlement and motor semantics as live simulation.
Disable births initially. Keep cues settled only for truly unchanged sources;
consumption, carrion creation and moving vibration sources still update them.
Training against an eternally emitting depleted patch would teach the wrong task.

Train two archetypes first: a blind chemical/contact ground body and a sighted
browser. Transfer compatible schemas/initializations into capgnawer and snail
arenas, then evaluate the different cue medium, diet and motor constraints.
Lanternjaw follows visible-body/strike support. Seedporter and bellwing wait for
climbing and flight/attachment actions. Do not multiply seven roles by all seeds
and all ablations at once.

Start within a detectable cue gradient and attainable feeding reach. Increase
starting distance, heading variation, obstacles and gaps only as performance
permits; include unrewarding/depleted cues and held-out layouts. Use actual intake,
energy cost and survival to distinguish partial success before full-horizon
survival. Inspect score/rank spread: rank ES cannot learn from all identical
scores, but all-starving episodes need not be identical. Do not feed target
coordinates, path distance, or privileged food stock into observations. Generalize
from frozen arenas to dynamic ecology before claiming lineage viability.

A diagnostic heuristic using exactly the same interface should first demonstrate
foraging in each arena. Failure means investigate sensors, actions, arena budget
and the heuristic; it does not prove no policy can solve the task. Success proves
one observable strategy exists, not that ES will discover it. Then perform one
informative ablation at a time: temporal versus paired chemistry; contact taste;
raw visual snapshots versus expansion; vibration versus no vibration.

**Budget from measurements.** The [tick profile](7_Research/voxel-tick-profile-2026-09-18.md)
contains the original 11.72 ms/tick and newer results. Its last listed rerun at
528a3ac reports 4.105 ms at one thread and 1.750 ms at sixteen, for that particular
coupled-world condition. These are historical measurements, not timings of our
proposed arenas or the latest concurrent checkout. 50 microseconds is a target.
The [R2d review](7_Research/r2d-forager-review-2026-09-15.md) records 8,452 episodes
for the completed seed-2 campaign; its first successful centre was generation 19,
not necessarily after all 8,452. Seed 1 first reached all training horizons at 58.
A 36,000-tick horizon is a historical task length, not a minimum for new arenas.

Measure single-arena wall time, setup/field settling, per-organ sensing and GRU
inference, then concurrent episode throughput on the actual worker count. Account
for all seeds, perturbations, centre evaluations and ablations. Do not divide a
multithreaded tick measurement by 24 as if each episode then used one core, or
assume linear scaling. Choose a bounded pilot from measured throughput before
scheduling a campaign. None is launched by this document.

## 7. Research checks and limits

The following evidence motivated the original proposals. Revised scalar counts,
field rules and founder assignments are engineering hypotheses; the papers do
not verify our training viability.

### 1. Littershredder — blind ground forager

fruit-fly larvae use concentration history while moving and
lateral head casts when turning; unilateral olfactory function can support
chemotaxis, with bilateral input improving aspects of performance. This supports
combining a small number of sampling points with temporal processing, rather
than assuming many compass directions are necessary.
[Gomez-Marin et al., 2011](https://www.nature.com/articles/ncomms1455).

### 2. Frondgrazer — low-foliage browser

locust experiments distinguish antennal contributions to
food-odor orientation from short-range mouthpart responses. This supports
separating finding vegetation from inspecting it at feeding reach.
[Zhang et al., 2017](https://pmc.ncbi.nlm.nih.gov/articles/PMC5485631/).
Locust visual experiments also support specialized processing of approaching
images; this is evidence for a looming cue, not a semantic predator detector.
[Chan and Gabbiani, 2013](https://journals.biologists.com/jeb/article/216/4/641/11866/Collision-avoidance-behaviors-of-minimally).

### 3. Seedporter — fruit visitor and climber

a field experiment on *Cynopterus sphinx* found visual cues
effective for locating fruits, with olfactory and haptic cues contributing to
evaluation and extraction. The result is species/context-specific; it does not
justify a universal rule that fruit scent always does the locating.
[Mahandran et al., 2021](https://doi.org/10.1016/j.beproc.2021.104426).

### 4. Bellwing — flower visitor

bumblebee experiments separated approach, landing, and
feeding responses: visual pollen cues could initiate approach, while combinations
of visual, olfactory, tactile, and gustatory cues supported more of the sequence.
[Wilmsen et al., 2017](https://doi.org/10.1002/ece3.2768).
Landing studies found control associated with optical expansion, supporting a
processed approach-motion input beyond flower recognition alone.
[Goyal et al., 2021](https://doi.org/10.1016/j.isci.2021.102407).

### 5. Capgnawer — blind fungal specialist

experiments on three springtail species showed odor-based
discrimination among fungi differing in secondary chemistry and grazing history.
This supports chemically distinguishable resources and imperfect food cues; it
does not imply a direct readout of toxicity or nutritional value.
[Staaden et al., 2011](https://doi.org/10.1016/j.soilbio.2010.10.002).

### 6. Ripple snail — blind wet-surface grazer

recordings from *Lymnaea stagnalis* tentacle nerves found
food-odor responses, with differing responses to tested odor sources. The study
supports tentacle chemosensation, but does not establish a reliable distant
biofilm detector or the information needed to map a water body.
[Ucciferri and Wyeth, 2023](https://doi.org/10.1111/ivb.12414).

### 7. Lanternjaw — ambush hunter with carrion fallback

mantis experiments demonstrate a specialized stereoscopic
mechanism based on temporal image change, rather than full primate-like image
matching. It can extract distance to moving targets under challenging image
conditions. This supports an organ producing useful motion/range features
without a general visual-cortex simulation.
[Nityananda et al., 2018](https://doi.org/10.1016/j.cub.2018.01.012).

### Substrate vibration addition

Experiments on a cerambycid beetle found vibration-evoked freezing, walking and
other responses, with femoral chordotonal organs implicated. This supports
assigning a substrate-sensitive mechanical organ. It does not establish that
intensity/trend identifies predators, that every blind animal has it, or that
our short-lived scalar field reproduces real wave propagation.
[Substrate vibrations mediate behavioral responses via femoral chordotonal organs
in a cerambycid beetle](https://pmc.ncbi.nlm.nih.gov/articles/PMC5002121/).

## 8. Handoff and later scope

Wrysk's process correction (2026-09-18): the review supplies the working direction;
do not take him through another organism-by-organism or parameter-by-parameter
approval loop. Resolve routine choices autonomously and retain their status as
testable defaults. His subsequent request was to finish this handoff and provide
implementation/testing plans. Phase-one design is now specified by those linked
plans; the broader catalogue remains an extensible working design, not canon.

Use temporal chemistry first, raw material-cone snapshots before motion tracking,
fixed founder schemas, surface-layer cue transport, and the two-archetype arena
sequence above. Keep the original littershredder's light patch and its lack of
Tremor. Start the frondgrazer with broad, coarse front/side coverage; adjust ray
placement and range through arena results. Optional organs elsewhere are later
variant experiments, not a checklist of questions for Wrysk. The implementation
plan supplies the first field coefficients and exact founder manifests. Work on
later roles only after the two-archetype results identify the next useful slice.

Later implementation checks should exercise anatomy gating, encoding stability,
sub-voxel gradients, contact-only taste, separate support layers, visual occlusion,
and pulse persistence over a controller interval. Keep ordinary tests to a few
frames or at most a few hundred ticks; longer learning studies are named studies,
not routine test-suite work. This revision changed only the design draft.
