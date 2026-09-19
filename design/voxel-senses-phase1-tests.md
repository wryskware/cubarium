---
design_status: exploration
last_reviewed: 2026-09-18
decision_refs: []
---

# Voxel senses — phase one testing, pilots, and outcomes

Companion to the [implementation plan](voxel-senses-phase1-plan.md); return to
[the sensing entry point](voxel-senses.md) for the design and research.
This is an execution plan for the future implementation round. No results below
are claimed to exist. Apply [WORKING_POLICY.md](../WORKING_POLICY.md): small
function tests, bounded studies, no golden-world hashes or archive/report suites.

## 1. Cheap checks before training

Most checks require one call or a few frames. Any ordinary test stepping the
simulation stays within 200 ticks and the repository's short-test time budget.
Longer episodes belong to explicit commands or ignored studies, not CI.

| Area | Small check and meaningful failure |
| --- | --- |
| Arena freeze | Over a few ticks, animal time advances and food can be consumed, while prepared terrain/water and unrelated plants do not evolve |
| Local motion | Forward/turn actions change only the body's resolved pose; a wall/drop constrains motion; turning and blocked attempts consume the motor budget |
| Paid food | Litter and foliage bites debit actual stocks and transfer organic material, mineral and energy once; failed/no-contact feeding transfers nothing |
| Feedback | Intake, structural loss, movement and contact outcomes are from the previous interval, reset once, and never include a prediction of success |
| Chemical field | One source creates a local response; removing it stops emission and leaves decaying residue; active spread cannot join a roof to the floor below |
| Chemical sampler | Two sub-voxel positions in one gradient cell differ under interpolation; crossing a wall does not interpolate its far-side value; birth creates no false trend |
| Encoding | Changing range alters what can be detected without renormalizing an already detected fixed-distance sample; response sensitivity changes are intentional receptor behavior |
| Touch/taste/water | Mouth taste is invalid before contact; contact location is correct after turning; dry is a valid zero; unreachable neighboring stocks never become mouth input |
| Light | Open and terrain-covered receptors differ; this first sky-only model does not report an emissive fungus |
| Material cone | A foreground rock blocks foliage/body hits; class distance belongs to that class; no-hit slots are zero; a tiny target missed by finite rays is measurable rather than magically detected |
| Geometry | Local pose, field sampling and rays wrap x consistently; separated support layers stay separated; a wrapped crossing cannot add a spurious turn |
| Policy boundary | Controller receives only the vector and its own memory; a no-food controller action cannot acquire a privileged target through its adapter |
| GRU shapes | One hand-computed forward step for a small shape, 23/3 and 37/3 shape acceptance, incompatible policy rejection, and valid tensor round-trip |
| ES plumbing | A tiny perturbation batch actually changes evaluated weights; cancellation within an episode counts work; fresh episodes reset hidden state and mutable arena data |

Do not test implementation details twice or pin exact trajectories. Numeric
conservation/encoding checks use appropriate tolerances; behavioral evidence is
statistical. A runtime schema digest protects meaning, not world reproducibility.
An independent test author checks the new model-rule functions as required by
working policy; there is no additional generic review or provenance suite.

Run `cargo nextest run -p <touched-crate>` for the crates actually changed.
The integrator runs `cargo nextest run --workspace --exclude cubarium-gpu` once
at final integration, as specified by the lean-orchestration policy. Do not run
any Rust suite for this planning-only change. Viewer verification is a short
explicit dev run, not a capture campaign.

## 2. Arena tasks and controls

Use four training layout seeds and eight untouched evaluation seeds per
archetype. A seed varies geometry/resource placement and starting heading within
the task, not its sensory contract. Evaluation seeds are not used to choose
parameters or the best generation. Source strengths and total food budgets must
remain declared rather than secretly rescuing a failed policy.

| Stage | Blind litter feeder | Sighted browser |
| --- | --- | --- |
| A: acquire | Start inside a litter gradient, off the edible patch, with varied heading | Start with visible low foliage, off feeding contact, with varied heading |
| B: persist/leave | Eat finite litter, then reach a second nearby cue patch | Crop one patch and reach another visible patch |
| C: distinguish | Decaying residue at a depleted patch and one contact obstacle | Visible out-of-reach foliage, a nearer accessible patch, and an occluder |

Training begins with A. Begin with 1,200 ticks (60 simulated seconds at 20 Hz),
which is ample for multiple body lengths without inheriting the old 36,000-tick
campaign. Adjust layout distances to the task and body rather than shortening
the body physics or granting special feeding reach. Starts are in signal, not
on food. Progress toward B only after repeated acquisition; C is diagnostic
stretch work in this phase. Do not demand that blind animals solve scent-free
maze navigation before their first reward.

At every stage compare:

- A stationary/no-intake controller, to reveal free intake or scoring artifacts.
- A stationary feeding controller, to show why merely remaining alive or feeding
  in place is insufficient from an off-food start.
- An observation-only heuristic. Blind version uses response/trend, local contact
  and its own turn memory; browser uses the fixed visual sectors and mouth/contact
  outcomes. Neither can read source coordinates, target Sites, a route, or a
  future-face reach query.

Before ES, the heuristic should acquire and consume food in at least three of
four varied training starts for each archetype. Also check that food consumed can
pay plausible upkeep/movement; foraging must not fail because litter is mineral-
poor relative to an unchanged leaf-eater physiology. Set the litter feeder's
digestive/assimilation configuration deliberately, once, using real `Taken`
composition. Do not add nutrients or convert carrion/wood into litter silently.
If the heuristic fails, diagnose and repair the fixture/interface/heuristic;
failure does not prove that every possible controller would fail.

For the pilot, rank candidates by settled assimilated intake minus paid motor
organic loss, divided by a fixed founder reference body mass, plus a small
survival term `0.25 × survived_fraction`. Keep components separately reported.
This is a capability-training score, not the world's reproductive fitness.
Check score variance before the first update; identical starvation scores offer
no ranking information. Resource positions/distance are not observation or
reward shortcuts. Terminal stores and full-horizon survival remain diagnostics.

## 3. Timing and bounded ES pilot

Measure arena construction/settling separately from tick cost. Measure at least
one and four animals, with heuristic and GRU separately, and account for field
updates and occupancy refresh after feeding. Report observations/second and
episodes/second, not just a world tick that excludes setup or inference.
Use one thread inside each small arena; compare one versus four episode workers.
Avoid nested 24-core episode and world parallelism. Reuse immutable prepared
layouts but reset all mutable stocks, cues, animal feedback and hidden state.

Use these default bounds for the first coordinated pilot, reducing work to fit
measured throughput rather than silently exceeding them:

| Item | Bound |
| --- | --- |
| Total timed studies | 20 minutes wall time, including timing, controls, training and evaluation |
| Per-archetype training | At most 8 minutes; one training seed, blind first, browser second |
| Episode workers | At most four, one simulation thread per episode |
| ES generation | Eight antithetic pairs on four training layouts: 64 perturbation episodes; four centre evaluations |
| Updates | At most 32 per archetype: 2,180 episodes including the initial centre, before controls/evaluation |
| Overall episode limit | 4,800 across both archetypes including smoke, controls, discarded/cancelled work and evaluation |
| Horizon | At most 1,200 ticks for this pilot; shorter smoke episodes are fine |
| Pilot output | One current/best policy per archetype and compact metrics; no frame captures or per-generation world dumps |

The 20-minute figure is a planning default for this later implementation round,
not a run launched now. Divide the time into up to eight minutes per training
pilot and four minutes for all other timed work; stop sooner at episode/update
caps. Before starting, reserve evaluation time. If the measured episode cost
cannot support a useful pilot inside these bounds, report the measured bottleneck
and reduce the work or improve the arena. Do not quietly train in the full
evolving world or claim the 50-microsecond target is achieved.

Save the chosen centre by training score, not the best sampled perturbation.
Evaluate it once on the eight held-out layouts and compare the same controls.
Use remaining allowance for at most one targeted diagnostic, such as hidden-state
reset or trend removal with the existing schema. Retraining paired chemical
organs or expansion needs another scoped experiment; do not automatically
multiply the whole campaign. No live sensor/body mutation is included here.

## 4. Expected goals and deliverables

**Required engineering deliverables:**

- Static-arena builder, explicit freeze schedule, finite food, and mutable cues
  after depletion, using the production sensory/motion/feeding functions.
- Blind 23-input and browser 37-input manifests with three local actions;
  correctly shaped GRU runtime, policy export/load, and voxel ES episode driver.
- Function tests and observation-only heuristic foraging results, plus measured
  setup/tick/sensor/GRU throughput and a bounded pilot outcome.
- Runnable check/bench/train/evaluate commands and an explicit viewer option for
  either archetype/controller. Existing interim presentation is sufficient.
- Brief handback: commits, commands, measured counts/time, pilot status, any
  saved policy, and one recommended next package. No separate report per worker.

**Learning target:** each selected centre acquires food from an off-food start
on at least six of eight held-out A layouts, exceeds the stationary-feeding
control in median net intake score, and continues operating through the episode.
Report intake and survival separately; a policy that consumes once and dies is
not sustained foraging. Reaching and feeding at a second depleted-patch successor
in B is the stretch goal. Eight layouts are a first useful screen, not a strong
statistical claim or ecological validation.

An implemented pipeline can pass its function checks while the learning target
fails. Report **engineering complete / learning target unmet**, with scores and
one evidence-based next change; do not call that trained foraging or conceal it
behind a successful training command. A pilot blocked by an invalid arena or an
unimplemented action is incomplete engineering, not merely a difficult learner.

After phase one, the orchestrator should be able to inspect both sensory worlds,
train/evaluate either founder without the full water/flora tick, and identify
whether a failure belongs to sensing, motor/food budget, or policy learning.
This is the platform for later dynamic-ecology transfer, vibration lineages and
other niches. It does not establish reproduction, sustainable coexistence,
flight/climbing/hunting, seven trained species, or a finalized production look.
