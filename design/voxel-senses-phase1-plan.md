---
design_status: exploration
last_reviewed: 2026-09-18
decision_refs: []
---

# Voxel senses — phase one implementation plan

Start from [voxel-senses.md](voxel-senses.md). This is the completed planning
handoff requested by Wrysk for an orchestrator to coordinate implementation.
The defaults below are concrete implementation targets, not claims of existing
features or promotion of the entire ecological design to canon. Resolve routine
implementation choices without another parameter-by-parameter approval loop.

Read the companion [testing and outcomes plan](voxel-senses-phase1-tests.md)
before assigning work. No implementation, training, or deployment was performed
in this planning session.

## Outcome and scope

Deliver two grounded foraging prototypes in a small static arena: a blind litter
feeder and a sighted foliage browser. Both must act through local sensory packets
and the same heading/effort/feeding interface. A diagnostic heuristic and a GRU32
must be interchangeable behind that interface. Provide bounded ES pilots,
evaluation, and an explicit development-viewer entry point.

The blind founder has simple light sensitivity and no Tremor. Phase one implements
Chem, Light, Contact, Wet, Taste, Self, and a small material Cone. Tilt, bilateral
chemistry, vibration, active appendage movement, visual tracking, flight, climbing,
hunting, and sexual reproduction remain later work. Implementing every catalogue
primitive would defeat the purpose of a first slice.

Phase one uses upright bodies on horizontal supports. A three-point articulated
support/tilt model is unnecessary for these arenas; omit Tilt from their schemas
instead of fabricating a slope signal. Thus the actual first schemas have **23
and 37 inputs**, not the broader catalogue's 26-value littershredder reference.
Adding Tilt later creates a new founder/schema as specified in the senses design.

## Current code and the missing bridge

These are source pointers checked during planning, not ownership of concurrent work.

| Area | Starting point | Work needed |
| --- | --- | --- |
| Voxel animal | `crates/cubarium-voxel-fauna/src/lib.rs`, `Animal` and `SpeciesConfig` | Present animal stores a support Site and physiology, not continuous heading/position or a neural controller |
| Existing behavior | `crates/cubarium-voxel-fauna/src/step.rs`, `plan_for`, `toward`, `act` | Replace target-driven planning for new controllers; no candidate-face food search or target-bearing helper may feed them |
| Food transfers | `crates/cubarium-voxel-flora/src/lib.rs`, `take_litter`, `take_foliage`, `Taken` | Reuse paid transfers and assimilation; add a litter-feeding fauna configuration, not a second resource ledger |
| Schedule | `crates/cubarium-voxel-sim/src/lib.rs`, `Sim`, `sys_fauna`, `sys_advance` | Explicit static-arena mode skips environmental evolution but advances animal/field time |
| GRU | `crates/cubarium-core/src/neural/gru.rs` | Reuse arithmetic with selectable I/O shapes; current interface is compiled for 70 inputs and 7 actions |
| ES | `crates/cubarium-search/src/es/{tensor,trainer,episode}.rs` | Shape-aware tensors and voxel episode driver; existing episode driver instantiates the flat World |
| Development view | `crates/cubarium/src/voxel/{mod,animal}.rs`, `scripts/run-voxel.sh` | Display the same arena/body positions and selected controller using existing interim presentation |

The old trainer's `MobileScript` is a route control and is unsuitable as this
phase's diagnostic heuristic. Existing flora stock queries may settle an actual
bite, but must not provide remote observations or choose a movement destination.

Coordinate with the active voxel water and flora/harness workers first. Their
performance work is independent; do not duplicate it or wait for a full-world
50-microsecond tick. Static arenas avoid that dependency. Do not alter
`design/handoffs/README.md` as part of this handoff.

## Frozen arena contract

- Dimensions: 32 × 16 × 12 voxels, width × height × depth, at 0.25 m/voxel.
  Preserve wrapped x and ordinary voxel support geometry. Default layouts keep
  food away from the seam; a specific seam layout checks geometry reuse.
- Terrain and water are prepared once. No rain, water solve, plant growth,
  litter decomposition, or spontaneous replenishment runs during an episode.
  Flora geometry is frozen except changes caused by actual feeding/removal.
- Animals still pay maintenance and movement, eat, assimilate, age and die.
  Births are disabled in arenas. Later live founders use the existing paid
  single-parent birth model; mate finding is outside this phase.
- Cue fields settle before the initial observation. Static background caches
  may be shared read-only across episodes. Resource stocks, cue evolution after
  feeding, animals, controller memory and feedback are private to each episode.
- Resource depletion stops emission and updates sensory occupancy. Remaining
  odor decays normally. Neither smell nor visual snapshots may keep reporting
  the initial food stock after it has been consumed.
- Use the same sensor, local motion and feeding code in the arena and normal
  voxel schedule. Freeze systems explicitly; do not introduce a different toy
  movement model for training.

Tick order: advance maintenance and age; update due cue/occupancy changes from
the previous tick; sample due controllers and consume prior-interval feedback;
hold their actions; resolve paid motion/contact and feeding; record outcomes and
deaths; advance time. All controllers sense the same pre-action state for a tick.

## Phase-one body and action contract

Add continuous x/z position and heading tied to the actual support Site. Start
with an upright disc/capsule footprint and a forward mouth/contact point. Use
local swept collision/support checks, with bounded steps to avoid tunneling.
No graph search, waypoint, nearest-food target, or autonomous turn-to-target.
Phase-one founders cannot step to a higher support; stairs/climbing are later.
At a drop or wall, movement is constrained and the actual local contact/delivery
feedback reports the result. Topology helpers must agree with the world and
presenter; do not import unrelated flat-face unfolding into the voxel ring.

Starting geometry: blind body length 0.125 m, browser 0.25 m; footprints fit their
length, with width half the length. Cruise is 1 BL/s, yaw cap 2 rad/s. Controller
period is 0.25 s at the current 20 Hz simulation tick; physics remains 20 Hz.
Place front/left/right contact receptors on the footprint boundary and an
underside support receptor. The mouth can contact only its actual local footprint
and short forward reach, initially 0.25 BL. These are tunable founder values,
not a claim that the current rendered glyph has this anatomy.

Three actions, held until the next controller call:

| Index | Action | Resolution |
| --- | --- | --- |
| 0 | Forward effort [0,1] | Requested forward speed × affordable motor capacity |
| 1 | Turn effort [-1,1] | Signed local yaw; no target argument |
| 2 | Feed effort [0,1] | Attempt the founder's supported local bite/handling operation |

For GRU logits, use sigmoid for forward/feed and tanh for turn, with a small
fixed deadband of 0.05 (absolute value for turn). This adapter is identical for
training and viewing; diagnostic controllers may emit already bounded actions.
Zero movement is rest; turning while stopped is permitted and paid. Forward
effort and turning share a motor budget using equivalent displacement
`abs(v) + body_radius × abs(yaw_rate)`. Attempted movement against a wall still
costs effort; collision cannot grant free searching. Add a named configurable
motor respiration coefficient and use the fauna's existing organic/energy loss
and mineral bookkeeping rather than creating a parallel energy store. Start
full-cruise motor respiration at the same rate as basal upkeep, then check the
arena's feeding budget. Record attempted versus delivered motion separately.

Organs occupy a declared share of paid body structure and add maintenance; no
per-read charge. A first constant sensor-tissue allocation of 5% of body structure
for the blind founder and 10% for the browser is a prototype cost setting. Given
a founder's core-structure budget and allocation f, total structure is
core / (1 - f); count sensor tissue within that total, not again in a second
ledger. Use the body's usual maintenance on the total. Adding organs therefore
raises paid structure/upkeep rather than freely relabeling existing tissue;
body additions must not come with free reserve or energy.
Freeze morphology/cost parameters during first ES pilots. Genetic variation and
cost optimization are later experiments, not prerequisites for foraging.

## Exact starting manifests

All scales and offsets are manifest data. Serialize ordered module definitions,
actions, physical reference scales, timing, and a schema version with policies.
Derive the policy digest from this contract. Reject incompatible policies; no
old-world migration or golden digest test is needed. Configuration coefficients
that alter query strength must not silently alter tensor order or meaning.
The new body layout also needs a fauna snapshot version change: refuse
incompatible old worlds and start fresh, rather than building a migration path.

| Module | Blind indices | Browser indices | Definition |
| --- | --- | --- | --- |
| Self | 0–7 | 0–7 | Energy, reserve, birth readiness, actual structural loss over interval, assimilated intake, resolved forward movement, resolved turn, motor delivery |
| Contact(4) | 8–12 | 8–12 | Front, left, right, underside contact, then validity |
| Wet | 13–14 | 13–14 | Immediate contact-water response, then validity |
| Taste(1) | 15–17 | 15–17 | Litter cue or foliage cue at actual mouth contact, resistance, then validity |
| Chem(litter) | 18–20 | — | Local response, smoothed trend, then validity |
| Light | 21–22 | — | Terrain-shaded ambient sky-light response, then validity |
| Cone(3, foliage/body) | — | 18–36 | Six values per sector, then one validity value |

`Self` structural loss is measured internal tissue loss, not a fictitious injury
model. Birth readiness is the current physiological threshold state even while
arena births are disabled. No predictor of future fitness is exposed. Normalize
physiology against fixed manifest references, never a population statistic.
Use initial adult body/energy/reserve references fixed for that founder schema.
Movement/turn use fixed cruise and yaw references × controller interval;
delivery is resolved/requested equivalent movement, or 1 when none requested.
Initial intake/loss/motion feedback is zero. Clamp all values and reject nonfinite
inputs rather than training through them.

Contact strengths may initially be binary geometric contact, not invented force.
Taste is invalid without mouth contact. Resistance is a fixed material-response
mapping measured at that contact, not inferred remote food quality. Dry Wet and
zero odor are valid zero readings. Light uses uniform sky illumination times
terrain exposure; canopy shading and emission are explicitly deferred in this
first scalar, so it cannot detect glowcaps.

Browser sectors have centres at yaw -60°, 0°, +60°, with a 3 × 3 fan of offsets
(-30°,0°,30°) yaw and (-20°,0°,20°) pitch: 27 rays total, 2 m maximum range and
a conservative fixed voxel-traversal cap. Each sector reports, in order:
clear fraction, mean all-hit proximity, foliage fraction, foliage mean proximity,
body fraction, body mean proximity. Proximity is `clamp(1 - distance/2 m, 0, 1)`;
2 m is the fixed encoding reference even if query range is later shortened.
Zero-hit attributes are zero, with presence carried by their fraction. Terrain,
wood, litter and water still occlude even though their identities are not exposed.
The eye has no memory, binocular reconstruction, expansion or body IDs. Validate
ray misses against small targets before increasing sample count.

The full reference littershredder includes Tilt; this first schema intentionally
does not. Broader schemas add rather than secretly populate unimplemented inputs.
With hidden width 32 and three outputs, parameter counts are **5,571** for 23
inputs and **6,915** for 37 inputs. These require the runtime/tensor adaptation;
they are not accepted by today's fixed-shape policy loader.

## Initial cue field settings

Implement only the litter-associated cue channel first. A support-layer node is
an exposed support face, not a whole column. Phase-one horizontal arenas connect
same-height neighboring support nodes through their local open near-surface
medium. Roof and floor never share a node. General vertical transport is deferred;
this initial approximation does not carry smell up stairs or through open air.

Use bilinear sampling among connected same-layer nodes, preserving barriers.
Handle unsupported/out-of-medium receptor positions as invalid. Cache terrain
connectivity once per arena; source strengths follow actual resource stocks.
There is no per-animal emitter scan.

Starting update: every 0.5 s, diffuse with a convex nearest-neighbor mix of 0.4
(missing blocked neighbors reflect the local value), decay with half-life 2 s,
and add local emission. Emission is `min(litter / M_emit, 1)` cue units/s;
`M_emit = 0.05` existing organic-material units. Discard values below 1e-5 after
the update. Maintain the active set plus its immediate neighbors. Specify the
operation order once and use it in both settling and live updates.
Settle for at most 120 field updates, stopping earlier when the maximum change
is below 1e-4 for eight consecutive updates. Charge that setup time to the
benchmark; reuse the prepared state only for unchanged source layouts.

Receptor response is `C / (1 + C)` initially; the saturation reference 1 is fixed
in the manifest. Smooth over tau = 1 s for the 1 BL/s founders; encode the
smoothed time derivative × 1 s, clamped to [-1,1]. Initialize from the first
sample. Readout has no privileged food direction. All these coefficients are
starting values to adjust within the bounded arena work, not new approval items.

## Work packages and coordination

Use existing persistent workers. One owner per area; resume by message rather
than spawning parallel agents on the same crate. The orchestrator integrates
and reads diffs/skeletons. Policy requires an independent test-authoring pass
for model-rule changes: scope it to the motion, sensory and ledger functions,
not an open-ended review round. High effort applies there; ordinary harness and
reporting work is medium. No Astra review round is implied by this plan.

| Package | Primary owner and files | Concrete return | Dependency |
| --- | --- | --- | --- |
| P1-A: arena and contract | Fauna/host owner; voxel-sim and voxel-search fixture entry | Static schedule mode, two manifests, finite consumable layouts; an idle body can step | First; coordinate shared sim file with performance worker |
| P1-B: body and local feeding | Same fauna owner; voxel-fauna, minimal flora boundary additions | Heading-based paid motion, contacts, feedback, litter feeder and browser intake through real transfers | A |
| P1-C: sensing | Same fauna owner; new focused sensory module near voxel-fauna, shared occupancy geometry where needed | Litter cue field, primitive samplers, bounded material cone, observation-only diagnostic controllers | A, B |
| P1-D: policy/ES adapter | Neural/search owner if already available, otherwise same worker sequentially; core neural and search ES | Shared GRU arithmetic with shape-aware tensors, voxel episode driver, load/save/cancellation | A's manifest; integrate after B/C |
| P1-E: tests and pilot | Scoped independent test author for model rules; search owner for runs | Short function tests, timing, heuristic outcomes, bounded ES pilots and held-out evaluation | B–D |
| P1-F: usable inspection and handback | Fauna/host owner; existing voxel viewer/launcher | Run either arena with heuristic or saved policy; observations in explicit diagnostics, concise delivery summary | C–E |

P1-D can proceed independently once manifests are fixed, if a suitable worker is
already assigned. Parallelism is optional. Do not create a new worker merely
because the package table has a row. Flora worker owns any changes to shared
food/occupancy geometry. Reuse the existing trainer's optimizer/rank logic and
GRU equations; avoid a second divergent neural implementation or general plugin
framework. Retain the flat adapter with its existing shape while making the
numerical core usable by the two voxel shapes.

Implement one discoverable command family in `cubarium-search` for arena check,
bench, train and evaluation. Exact spelling can follow the current CLI; the
implementer records runnable commands in the handback. Require arena/founder,
controller, seed, episode limit, worker count and wall-time cap as applicable.
Add an explicit development-viewer arena/controller selection. Reuse interim
assets and [art direction](art-direction/Cubarium_Art_Direction_v0.1.md); do not
generate production art. Use `--background`/`CUBARIUM_FLOAT=1` for windows opened
autonomously. No automatic replacement of the user's ambient scene is required.

## Phase-one handback

The orchestrator returns runnable commands; commits; both manifest sizes and
action semantics; which function checks passed; heuristic feeding results;
measured arena/setup/sensor/inference throughput; compute actually spent; and
held-out results for any selected policies. Keep the return brief and link the
current policy/metrics output. Use one normal build cache and a small disposable
run directory; source history belongs in Git, not frozen binaries or captures.

Use [the tests/outcomes plan](voxel-senses-phase1-tests.md) to distinguish an
implemented training pipeline from demonstrated learned foraging. Success is
useful local behavior from honest inputs; sustainable ecology, seven trained
species and live co-evolution are not phase-one deliverables.
