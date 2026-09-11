---
design_status: exploration
last_reviewed: 2026-09-11
decision_refs: []
---

# Pre-implementation plan review (2026-09-11)

Reviewer: Claude Fable 5.1, at Wrysk's request. Scope: every document in this
vault at commit `959a2c9`, read in full, plus the display shim at revision
`7a21b5f`. This is evidence for Wrysk's decisions. It changes no document
status and proposes no ledger entry. Findings are ordered by how much they
change the odds of the M3 and M4 exit evidence being met.

**Verdict.** The geometry and the accounting/transaction discipline are the
strongest parts of the plan and should be built as written. The ecology is
plausible, but three attractors (a minimal grazer, a sessile autotroph, and
cube-wide boom-bust synchrony) are more likely than the risk register
suggests, and the recovery layer is deep enough to hide all three. The single
biggest risk to the brief's "recognizable habits" goal is the dense recurrent
controller: at the generation rates the plan itself projects, it cannot be
expected to produce inherited behavior a viewer could name. M4 stacks nine
mechanisms into one milestone, which will make results unattributable.

## F1. The recurrent controller will not yield readable inherited habits at the projected generation rate

Refs: [evolution](../evolution.md) "Controller proposal" and "Showing that
evolution matters"; [ecology](../ecology.md) time-scale table;
[brief](../brief.md) "Prefer genes with observable consequences".

| Quantity | From the plan | Consequence |
| --- | --- | --- |
| Generation time | 10 to 90 min | 16 to 144 generations per day; 100 to 1,000 per week |
| Live population | at most 512, about 72 seeded, local budding | effective population well under 200 |
| Controller parameters | 320 weights plus 8 biases plus 8 time constants | every birth perturbs about 336 dimensions |
| Mutation style | small perturbations to most weights | selection can act on only a few dimensions per generation |

With those numbers there are two outcomes and neither is the one the plan
wants. If the mutation step is small, purifying selection dominates: descendants
behave like the hand-designed founder plus noise, and the "ancestor/descendant
samples demonstrate inherited changes" criterion in
[implementation-plan](../implementation-plan.md) M3 fails or is satisfied only
by pigment drift. If the step is large enough to explore weight space, most
children are broken, and "mutated populations remain viable" fails. In neither
case can an observer say "this lineage evolved longer pauses", because a change
in a dense weight matrix has no name. The document already lists the right
fallback ("reduce its dimensions or introduce structured sensory-to-action
biases"). That fallback should be the default.

**Alternative: a reflex genome first.** Outputs are inherited gains on a fixed
basis of roughly 12 to 20 named drives: food-gradient taxis, light and moisture
preference taxis, waste avoidance, rim avoidance, approach or avoid by relative
body size, signal following, rest threshold on reserve, feeding persistence,
flee-on-damage gain, gait oscillator period and amplitude, budding threshold,
pause duration. Add two inherited leaky integrators (hunger memory, alarm
memory) for hysteresis. Every gene is monotone and nameable, founders are a
short list of numbers rather than hand-tuned matrices, the observer can
histogram genes directly, and mutants are viable by construction. Keep the
8-unit recurrent network as a later experiment layered on top of the reflex
outputs, added only if experiment E5 below shows reflex mutants are too tame.
This narrows the README's "small inherited recurrent controller" rather than
abandoning it, and it is the cheapest change with the largest effect on M3.

## F2. Two dominant strategies the risk register underweights

Refs: [ecology](../ecology.md) "Organisms and interactions";
[evolution](../evolution.md) trait table; [implementation-plan](../implementation-plan.md)
risk register.

**The minimal grazer.** Small-step mutation moves every allocation a little
each birth, and every step toward smaller, cheaper, and dumber is selected as
long as a random walk still finds food. The plan's "cheap inactivity must not
make expensive unused organs free" rule addresses unused organs, not absent
ones. The only counterforce in the current design is reserve capacity across
lean intervals, which bites only if producer recovery (2 to 15 min) is slow
relative to travel time between patches. That is a tuning regime, not a
structural guarantee, and predation cannot rescue it because predators arrive
later and persist poorly (see F3). This is the most likely reason the cube ends
up as a field of identical dots.

Concrete alternatives: make feeding rate on a contested cell scale with mouth
investment so a larger mouth wins the patch rather than merely eating faster;
make a stable body-size polymorphism under grazing alone (no predators) an
explicit M3 exit criterion; run E4 before any predation work.

**The sessile autotroph.** Fan allocation, zero movement, and budding in place
compete with `P0`/`P1` for nutrient and light, cost little to maintain, and are
eaten by nobody until predators exist. This tiles the lit regions, stops the
substrate under it from producing, starves grazers, and leaves a static display
of frills. Concrete alternatives: give organisms no photosynthesis until M4
(the M3 fan is drag, exposure, and appearance only); when added, cap organism
light efficiency strictly below the producer pools; make fan area grazable so
the fan is edible surface, not free income.

**The rim strip** is a niche, not a failure. With any innate rim aversion the
bottom 10 to 12 pixels of each side face are under-grazed, producers accumulate
there, and a lineage that ignores the rim gets a reserve. This is a good thing
only if rim aversion is heritable and can fall to zero, as the topology
document already allows. See F7.

## F3. Cube-wide synchrony is the likely collapse loop

Refs: [ecology](../ecology.md) "Habitat and time scales", "Why a successful
strategy might stop being successful"; [architecture](../architecture.md)
capacity table.

A 12-pixel sensing radius covers about a ninth of a face, movement is fast
relative to patch spacing, and nutrient diffuses. Those three couple every
patch to every other. The expected trajectory from 72 seeds is a bloom to the
512 cap within the first hour, near-global depletion, mass starvation, a
detritus pulse, then a long recovery, repeating roughly once per generation.
Viewed from the office that reads as "everything appears, everything dies".
The plan's "no single global season" rule handles weather-driven synchrony but
not organism-driven coupling. Predators in small synchronous worlds go extinct
on the first crash, which is why "hunters die before getting descendants"
should be expected rather than treated as a tuning risk.

Concrete alternatives: add a static niche gradient derived from the embedding's
Y coordinate (Top bright and dry, lower rim dim and wet), which is free given
the [topology](../surface-topology.md) embedding and makes different regions
peak at different times; keep organism speed low relative to patch spacing;
set an ecological target density (100 to 200 live organisms, 20 to 40 per face)
so the cap is never the density regulator; treat specialist predation as an
episodic event rather than a persistent guild, and make flesh digestion a
facultative allocation every grazer can carry a little of. Track the
coefficient of variation of population and the cross-correlation of per-face
populations as first-class M2 telemetry.

## F4. The recovery layer can hide F2 and F3

Refs: [ecology](../ecology.md) "Continuity, dormancy, and rare recovery";
[implementation-plan](../implementation-plan.md) M4 controls.

There are three mechanisms: paid dormant propagules, historical-library
recruitment, and founder-reservoir immigrants. Only the first is ecological.
The library recruitment is the most complex of the three and the one most able
to make a collapsing world look like it has turnover. Collapse the last two into
one loudly logged reseed-on-extinction, keep the 128-entry library as an
observer-side archive only, and add a no-dormancy control to the M4 list.
Dormancy is a legitimate storage effect, but it can also carry a lineage
through a dynamic that would otherwise fail, so it must be toggleable in
evaluations. The 8-organisms-for-10-minutes trigger is far below the visual
floor; that is fine for an unattended fallback, but without an explicit target
density the telemetry cannot distinguish "quiet" from "collapsed".

## F5. M4 is overloaded; scavenging belongs in M2

Refs: [implementation-plan](../implementation-plan.md) M2 and M4.

M4 introduces the second producer pool, scavenging, predation, defense,
stress, signals, slow forcing, propagules, library recovery, and optionally
patch improvement, then evaluates 12 seeds for 24 hours with four controls.
"One mechanism at a time" inside that milestone is weeks of evaluation, and the
exit evidence is written for the combined system. Move scavenging into M2: it
is the same contact-and-feed code applied to `D`, it closes the recycling loop
visibly, and it is the first succession the cube can show. Move slow forcing
into M2 as part of weather. Move facultative predation into M3 alongside the
reflex genome. Leave `P1`, waste, signals, and patch improvement in M4 as
optional additions, each with a written prediction that a control can refute.

## F6. Geometry is sound and verified; four additions

Refs: [surface-topology](../surface-topology.md) throughout;
[local-contracts](local-contracts.md).

Checked during this review: the embedding table, the eight-seam transport
table, the along-edge parameter maps, the three worked continuous examples,
and the quarter-turn rule were derived independently from the embedding and
agree with `Face::neighbor`, `cross_seam`, `rotate_heading`, and
`pixel_direction` in the shim's `geometry.rs`, including the fact that the
same rotation carries both the outward direction and the along-edge tangent.
The parallel-transport statements (seam and back is identity; a loop around a
top vertex rotates by a quarter turn) are correct. The face-graph statement
that a top vertex adds no diagonal edge is correct; the three cells at a vertex
are pairwise adjacent, which is the curvature and is harmless for scalars.

Additions:

- **Name the slow oracle.** The document asks for an independent slow reference
  without saying what it is. Use the 3D embedding: unfold by rotating about
  the shared edge in 3D, compute chord length in the unfolded plane, and
  enumerate unfoldings by face path. This is the natural cross-check for both
  transport and distance and is a few dozen lines.
- **Use the chord as the broad phase.** The document rightly says the chord is
  not the interaction metric because it can cut through the cube. It is a
  valid lower bound on surface distance, so rejecting pairs whose chord exceeds
  the radius is exact with no false negatives. At 512 organisms that is about
  131,000 chord checks per tick, trivial in Rust, and it removes the need for
  the coarse bins and their seam-aware neighbor-image bookkeeping at M2.
- **Add isotropy tests.** Per-seam fixtures catch local errors; global bias
  needs a global test. Run many random walkers with a reflecting rim for many
  steps and require flat per-cell occupancy; diffuse a delta from a top-vertex
  cell and require symmetry across the three faces. Reversal and rotation bugs
  that pass every unit test show up here.
- **Define vertex render ownership.** A rigid body straddling a top vertex has a
  90-degree missing wedge; it cannot be drawn consistently. Assign each
  destination pixel to the unfolding with the shortest surface path from the
  body anchor, accept a visible tear for the second the body spans the vertex,
  and put that fixture in M1 as already planned.

Two smaller notes. The bottom rim is intrinsically straight even at bottom
corners (unfolding two side faces about their shared edge gives one straight
boundary line), so rim reflection is a v-component reflection in the current
chart with no corner case; test a single step that both reflects and crosses a
vertical seam. The shim's Top-face quarter-turn correction recorded in its
architecture document is layout, not client contract; Cubarium must not
compensate for it, and the M1 physical check covers this.

## F7. Rim: start with pure reflection

Refs: [surface-topology](../surface-topology.md) "The open bottom and exact
corners"; [evolution](../evolution.md) observation groups.

The plan builds rim sensing into the observation vector and steering. Start
without it: a reflecting boundary with no sensory term, watched on the cube for
ten minutes. A bounce off glass is familiar and cheap. Add rim proximity as a
heritable reflex gain only if the bounce looks wrong or organisms pile up on
the rim. This removes one observation channel and one steering term from M2
and keeps the rim strip an evolvable niche rather than a designed exclusion.

## F8. Simplifications ranked by benefit over cost

| Mechanism | Document | Recommendation |
| --- | --- | --- |
| Dense recurrent controller | evolution | Reflex genome first; network as a later modulator (F1) |
| Three recovery mechanisms | ecology | One paid dormancy plus one logged reseed (F4) |
| Two signal channels | ecology | Zero until an aggregation benefit is measured; then one |
| Waste field `W` | ecology | Defer; test whether depletion plus detritus give enough memory |
| Four scheduling rates with ID-phase staggering | architecture | One world rate at M2; stagger only on profiling evidence |
| Coarse spatial bins | surface-topology, architecture | Chord-bound all-pairs at this scale (F6) |
| Ten visible trait rows | evolution | Three morphology axes plus gait for M3 (F9) |
| Rim sensing | surface-topology | Pure reflection first (F7) |
| Stress-modulated mutation rate | evolution | Drop; it confounds attribution |
| Reproducible sensor noise | evolution | Drop initially |
| Trails in world state | architecture | Render-only, derived from a short position history in the render view |
| Two visibly distinct substrate colors | ecology, appearance | Keep `P1` as an M4 experiment; drop the visual-distinction goal at dark levels |
| Seam-aware bilinear substrate interpolation | surface-topology | Nearest cell plus a one-pixel seam-aware blur is enough at 4-pixel cells |

## F9. Visible traits: pick three shape axes and let motion carry the rest

Refs: [evolution](../evolution.md) trait table; [appearance](../appearance.md)
"Visual hierarchy".

At 3 to 7 pixels, fan area, armor rim pixels, antennae, and mouth type are all
"a slightly different blob". Motion is the readable channel at this
resolution: cadence, speed, pause length, turning style. For M3 choose lobe
count and length, one head-or-tail appendage type, and an inherited hue
accent as the shape axes, and map the remaining traits to gait parameters.
Run E6 on the physical cube before coupling the full grammar to physiology.

## Experiments to run first

Ordered so that each can disprove an assumption the later ones depend on.

| ID | When | Experiment | Metric | Disproves |
| --- | --- | --- | --- | --- |
| E1 | M1 | Random-walker occupancy with reflecting rim; delta diffusion from a top-vertex cell | Flat per-cell histogram; three-way symmetry | Transport or reversal bugs invisible to per-seam tests |
| E2 | M2 | One grazer, no mutation; sweep producer regrowth time, nutrient diffusion, speed | Autocorrelation length of `P` and organisms; population coefficient of variation; per-face cross-correlation | "Local depletion creates moving fronts" (F3) |
| E3 | M2 | Count births per hour per lineage | Generations per day | The "days to diverged descendants" timescale (F1) |
| E4 | M3 lite | Mutation on size, metabolism, sensing radius, reserve only; no predation; 12 seeds by 24 h | Body-size and sensing distributions against their lower bounds | "Tradeoffs plus depletion sustain size variety" (F2) |
| E5 | Before M3 | 1,000 mutants of a viable founder for each candidate controller, each run alone in a neutral arena for one lifetime | Fraction that feed and bud; count of nameable behavior changes | "Small controller mutations stay viable and legible" (F1) |
| E6 | Before M3 physiology | Twelve grammar samples on the physical cube; two viewers asked which are related and which is the hunter | Correct-pairing rate | "Ten visible traits" (F9) |
| E7 | M3 or M4 start | Fixed hunter founder in a stable grazer world; no mutation, no recovery; 12 seeds | Time to hunter extinction | "Specialist predation persists" (F3) |
| E8 | M1 or M2 | Pure reflection, no rim sense, watched on the cube | Judgement | "Rim sensing is needed" (F7) |
| E9 | M4 | All assistance off (dormancy, library, immigrants) | Time to mobile extinction, median over seeds | "The ecology is self-sustaining" (F4) |

E1, E2, E4, and E5 are the ones to run before committing to the M3 design.

## Keep as written

The material and energy split with named sources and sinks; request-then-settle
transactions from a pre-transfer state; paid births with escrow; no fitness
function and no species quotas; the deterministic single-threaded core with
partitioned random streams; the checkpoint and journal design; the full
topology acceptance list; assistance tagged and disabled in evaluations; the
refusal to draw five independent sprites; the M1 ordering that proves geometry
before ecology can hide it.
